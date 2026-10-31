# /// script
# requires-python = ">=3.11,<3.14"
# dependencies = ["praat-parselmouth>=0.4.5", "numpy>=1.26", "imageio-ffmpeg"]
# ///
"""Where do pitch breaks sit: share within 120 ms of a speech onset/offset (silence edge >=150 ms)."""
import json, sys, subprocess
from pathlib import Path
import numpy as np, parselmouth, imageio_ffmpeg
SR = 24000
def decode(p):
    cmd=[imageio_ffmpeg.get_ffmpeg_exe(),"-loglevel","error","-i",str(p),"-ac","1","-ar",str(SR),"-f","f32le","-"]
    return np.frombuffer(subprocess.run(cmd,capture_output=True,check=True).stdout,dtype=np.float32)
root=Path(sys.argv[1])
for cond in sys.argv[2:]:
    tot=edge=0; voiced=0
    for pas in sorted((root/cond).iterdir()):
        for f in sorted(pas.glob("*.webm")):
            x=decode(f).astype(np.float64)
            p=parselmouth.Sound(x,SR).to_pitch_ac(time_step=0.01,pitch_floor=60,pitch_ceiling=600)
            fr=p.selected_array["frequency"]; t=p.xs()
            v=fr>0; st=12*np.log2(np.where(v,fr,1)/100)
            jb=np.where(v[1:]&v[:-1]&(np.abs(np.diff(st))>4))[0]
            # rms frames 10ms
            hop=240; n=len(x)//hop
            db=20*np.log10(np.sqrt(np.mean(x[:n*hop].reshape(n,hop)**2,axis=1))+1e-9)
            sil=db< -45
            # silence runs >=15 frames -> edges
            edges=[]; i=0
            while i<n:
                if sil[i]:
                    j=i
                    while j<n and sil[j]: j+=1
                    if j-i>=15: edges += [i*0.01, j*0.01]
                    i=j
                else: i+=1
            edges=np.array(edges) if edges else np.array([-9.0])
            for k in jb:
                tt=t[k]; tot+=1
                edge += np.min(np.abs(edges-tt))<0.12
            voiced+=v.sum()*0.01
    print(f"{cond:10s} breaks {tot:4d} per_min {60*tot/voiced:5.1f} near_edge {edge/max(1,tot):.2f} edge_per_min {60*edge/voiced:5.1f} interior_per_min {60*(tot-edge)/voiced:5.1f}")
