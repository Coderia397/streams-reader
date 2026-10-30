// AudioWorklet processor: copies mono input to the main thread in ~2048-sample chunks with RMS and peak for the meter.
class NolabsRecorder extends AudioWorkletProcessor {
  constructor() {
    super();
    this.buf = new Float32Array(2048);
    this.n = 0;
  }

  flush() {
    if (this.n === 0) return;
    const samples = this.buf.slice(0, this.n);
    let sum = 0;
    let peak = 0;
    for (let i = 0; i < samples.length; i++) {
      const v = samples[i];
      sum += v * v;
      const a = v < 0 ? -v : v;
      if (a > peak) peak = a;
    }
    this.port.postMessage({ samples, rms: Math.sqrt(sum / samples.length), peak }, [samples.buffer]);
    this.n = 0;
  }

  process(inputs) {
    const input = inputs[0];
    const ch = input && input[0];
    if (ch) {
      for (let i = 0; i < ch.length; i++) {
        this.buf[this.n++] = ch[i];
        if (this.n === this.buf.length) this.flush();
      }
    }
    return true;
  }
}

registerProcessor("nolabs-recorder", NolabsRecorder);
