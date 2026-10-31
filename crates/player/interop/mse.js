// @ts-check
/**
 * MediaSource glue for the nolabs player: Promise wrappers around SourceBuffer's event API. All decisions (what to
 * append, when to evict, where to seek) are made in Rust; this module only turns events into Promises.
 */

/** Rejects an open() that never sees `sourceopen` (the element was given another source first). */
const OPEN_TIMEOUT_MS = 10000;

/**
 * @param {string} mime
 * @returns {boolean}
 */
export function mseSupported(mime) {
  return typeof MediaSource !== 'undefined' && MediaSource.isTypeSupported(mime);
}

export class MseSession {
  /**
   * @param {MediaSource} source
   * @param {SourceBuffer} buffer
   */
  constructor(source, buffer) {
    /** @private */ this.source = source;
    /** @private */ this.buffer = buffer;
    /** @private */ this.closed = false;
  }

  /**
   * Attaches a new MediaSource to `media` and resolves with a session once its single SourceBuffer exists, in
   * `sequence` mode so every append lands right after the previous one whatever its own timestamps say.
   * @param {HTMLMediaElement} media
   * @param {string} mime
   * @returns {Promise<MseSession>}
   */
  static open(media, mime) {
    return new Promise((resolve, reject) => {
      const source = new MediaSource();
      const url = URL.createObjectURL(source);
      const timer = setTimeout(() => {
        URL.revokeObjectURL(url);
        reject(new DOMException('MediaSource did not open', 'AbortError'));
      }, OPEN_TIMEOUT_MS);
      source.addEventListener(
        'sourceopen',
        () => {
          clearTimeout(timer);
          // The element holds the attachment now; the URL is no longer needed.
          URL.revokeObjectURL(url);
          try {
            const buffer = source.addSourceBuffer(mime);
            buffer.mode = 'sequence';
            resolve(new MseSession(source, buffer));
          } catch (err) {
            reject(err);
          }
        },
        { once: true },
      );
      media.src = url;
    });
  }

  /**
   * Appends one complete WebM file. Resolves with the media time where it ends: the end of the buffer, since sequence
   * mode lays every append right after the previous one and eviction only removes from the front. Not
   * `timestampOffset`: after an append that holds the offset applied to it, which is where it starts (Chrome 158
   * measured by crates/player/harness: after appending 2.741 s then 17.361 s it reads 0 then 2.741). Rejects with the
   * browser's error, whose `name` is `QuotaExceededError` when the buffer is full.
   * @param {Uint8Array} bytes
   * @returns {Promise<number>}
   */
  append(bytes) {
    return this.run(() => this.buffer.appendBuffer(bytes)).then(() => this.end());
  }

  /**
   * End of the last buffered range in seconds, NaN when nothing is buffered.
   * @private
   * @returns {number}
   */
  end() {
    try {
      const ranges = this.buffer.buffered;
      return ranges.length ? ranges.end(ranges.length - 1) : NaN;
    } catch {
      return NaN;
    }
  }

  /**
   * @param {number} start seconds
   * @param {number} end seconds
   * @returns {Promise<void>}
   */
  remove(start, end) {
    return this.run(() => this.buffer.remove(start, end));
  }

  /**
   * Buffered ranges flattened as [start0, end0, start1, end1, …] in seconds.
   * @returns {Float64Array}
   */
  buffered() {
    if (this.closed) return new Float64Array(0);
    try {
      const ranges = this.buffer.buffered;
      const out = new Float64Array(ranges.length * 2);
      for (let i = 0; i < ranges.length; i++) {
        out[2 * i] = ranges.start(i);
        out[2 * i + 1] = ranges.end(i);
      }
      return out;
    } catch {
      // The SourceBuffer was removed from its MediaSource.
      return new Float64Array(0);
    }
  }

  /** Marks the stream complete so the element fires `ended` at its end. Safe to call more than once. */
  finish() {
    if (this.closed || this.source.readyState !== 'open' || this.buffer.updating) return;
    try {
      this.source.endOfStream();
    } catch {
      // Raced with a teardown; nothing to finish.
    }
  }

  /** Stops any pending operation. The element's source is replaced or removed by the caller. */
  close() {
    this.closed = true;
    try {
      if (this.source.readyState === 'open' && this.buffer.updating) this.buffer.abort();
    } catch {
      // Already detached.
    }
  }

  /**
   * Runs one SourceBuffer operation and settles on its outcome.
   * @private
   * @param {() => void} start
   * @returns {Promise<void>}
   */
  run(start) {
    if (this.closed) return Promise.reject(new DOMException('session closed', 'AbortError'));
    const buffer = this.buffer;
    return new Promise((resolve, reject) => {
      /** @param {() => void} settle */
      const once = (settle) => () => {
        buffer.removeEventListener('updateend', ok);
        buffer.removeEventListener('error', failed);
        buffer.removeEventListener('abort', aborted);
        settle();
      };
      const ok = once(resolve);
      const failed = once(() => reject(new DOMException('the browser could not decode the audio', 'DecodeError')));
      const aborted = once(() => reject(new DOMException('append aborted', 'AbortError')));
      buffer.addEventListener('updateend', ok);
      buffer.addEventListener('error', failed);
      buffer.addEventListener('abort', aborted);
      try {
        start();
      } catch (err) {
        buffer.removeEventListener('updateend', ok);
        buffer.removeEventListener('error', failed);
        buffer.removeEventListener('abort', aborted);
        reject(err);
      }
    });
  }
}
