// Microphone capture for voice cloning. Browser processing (echo cancellation, noise suppression, auto gain) is off:
// it smears the timbre the clone is built from. Samples arrive from an AudioWorklet (assets/worklets/recorder-worklet.js)
// in small Float32 chunks; each chunk also reports its RMS and peak for the live meter.

let session = null;

export async function startRecording(onLevel) {
  if (session) await cancelRecording();
  const stream = await navigator.mediaDevices.getUserMedia({
    audio: { echoCancellation: false, noiseSuppression: false, autoGainControl: false, channelCount: 1 },
  });
  const ctx = new AudioContext();
  try {
    await ctx.audioWorklet.addModule("/app/assets/worklets/recorder-worklet.js");
    const source = ctx.createMediaStreamSource(stream);
    const node = new AudioWorkletNode(ctx, "nolabs-recorder", {
      numberOfInputs: 1,
      numberOfOutputs: 1,
      channelCount: 1,
      channelCountMode: "explicit",
    });
    // A muted path to the destination keeps the graph pulling the worklet on every browser.
    const mute = ctx.createGain();
    mute.gain.value = 0;
    const chunks = [];
    node.port.onmessage = (e) => {
      chunks.push(e.data.samples);
      onLevel(e.data.rms, e.data.peak);
    };
    source.connect(node);
    node.connect(mute);
    mute.connect(ctx.destination);
    if (ctx.state === "suspended") await ctx.resume();
    session = { stream, ctx, node, source, chunks };
    return ctx.sampleRate;
  } catch (err) {
    stream.getTracks().forEach((t) => t.stop());
    await ctx.close();
    throw err;
  }
}

async function teardown() {
  const s = session;
  session = null;
  if (!s) return null;
  s.node.port.onmessage = null;
  s.source.disconnect();
  s.node.disconnect();
  s.stream.getTracks().forEach((t) => t.stop());
  const sampleRate = s.ctx.sampleRate;
  await s.ctx.close();
  return { chunks: s.chunks, sampleRate };
}

// Returns { samples: Float32Array, sampleRate }.
export async function stopRecording() {
  const done = await teardown();
  if (!done) return { samples: new Float32Array(0), sampleRate: 48000 };
  const total = done.chunks.reduce((n, c) => n + c.length, 0);
  const samples = new Float32Array(total);
  let at = 0;
  for (const c of done.chunks) {
    samples.set(c, at);
    at += c.length;
  }
  return { samples, sampleRate: done.sampleRate };
}

export async function cancelRecording() {
  await teardown();
}
