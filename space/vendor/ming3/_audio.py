"""Small torchaudio replacements (load / Resample / save).

torchaudio stopped publishing wheels after 2.11, so this keeps the runtime on
the torch that the host image ships. The resampler is a port of torchaudio
v2.7.1 ``functional._get_sinc_resample_kernel`` / ``_apply_sinc_resample_kernel``
with the same defaults as ``torchaudio.transforms.Resample``.
"""

from __future__ import annotations

import math

import numpy as np
import soundfile as sf
import torch


def load(path: str):
    data, sample_rate = sf.read(path, dtype="float32", always_2d=True)
    return torch.from_numpy(np.ascontiguousarray(data.T)), int(sample_rate)


def save(path: str, waveform: torch.Tensor, sample_rate: int) -> None:
    sf.write(path, waveform.detach().float().cpu().numpy().T, int(sample_rate))


def _sinc_kernel(orig_freq, new_freq, gcd, lowpass_filter_width=6, rolloff=0.99):
    orig_freq = int(orig_freq) // gcd
    new_freq = int(new_freq) // gcd
    base_freq = min(orig_freq, new_freq) * rolloff
    width = math.ceil(lowpass_filter_width * orig_freq / base_freq)
    idx = torch.arange(-width, width + orig_freq, dtype=torch.float64)[None, None] / orig_freq
    t = torch.arange(0, -new_freq, -1, dtype=None)[:, None, None] / new_freq + idx
    t *= base_freq
    t = t.clamp_(-lowpass_filter_width, lowpass_filter_width)
    window = torch.cos(t * math.pi / lowpass_filter_width / 2) ** 2
    t *= math.pi
    scale = base_freq / orig_freq
    kernels = torch.where(t == 0, torch.tensor(1.0).to(t), t.sin() / t)
    kernels *= window * scale
    return kernels.to(dtype=torch.float32), width


def resample(waveform: torch.Tensor, orig_freq: int, new_freq: int) -> torch.Tensor:
    if orig_freq == new_freq:
        return waveform
    gcd = math.gcd(int(orig_freq), int(new_freq))
    kernel, width = _sinc_kernel(orig_freq, new_freq, gcd)
    kernel = kernel.to(device=waveform.device, dtype=waveform.dtype)
    o, n = int(orig_freq) // gcd, int(new_freq) // gcd
    shape = waveform.size()
    waveform = waveform.reshape(-1, shape[-1])
    num_wavs, length = waveform.shape
    waveform = torch.nn.functional.pad(waveform, (width, width + o))
    out = torch.nn.functional.conv1d(waveform[:, None], kernel, stride=o)
    out = out.transpose(1, 2).reshape(num_wavs, -1)
    target_length = int(math.ceil(n * length / o))
    out = out[..., :target_length]
    return out.view(shape[:-1] + out.shape[-1:])
