// @ts-check
/** DOM helpers for the nolabs player that web-sys does not cover or that are simpler to express here. */

/**
 * Creates the player's hidden <audio> element and attaches it to the document, so the page keeps one long-lived
 * media element (the one Android ties the media notification and background playback to).
 * @returns {HTMLAudioElement}
 */
export function createHiddenAudio() {
  const audio = document.createElement('audio');
  audio.preload = 'auto';
  audio.hidden = true;
  audio.setAttribute('aria-hidden', 'true');
  setPreservesPitch(audio);
  document.body.appendChild(audio);
  return audio;
}

/**
 * Keeps the voice's pitch at every playback rate. Older engines only know the prefixed names.
 * @param {HTMLMediaElement} media
 */
function setPreservesPitch(media) {
  const m = /** @type {any} */ (media);
  m.preservesPitch = true;
  m.mozPreservesPitch = true;
  m.webkitPreservesPitch = true;
}

/**
 * Starts playback. Resolves with "" on success or the error's name (`NotAllowedError` when the browser blocks it,
 * `AbortError` when a new source interrupted it); never rejects.
 * @param {HTMLMediaElement} media
 * @returns {Promise<string>}
 */
export function playMedia(media) {
  try {
    return media.play().then(
      () => '',
      (err) => errorName(err),
    );
  } catch (err) {
    return Promise.resolve(errorName(err));
  }
}

/**
 * The source switch in progress per element (fallback path): at most one, so a superseded switch can never seek or
 * start the element once a newer source loads.
 * @type {WeakMap<HTMLMediaElement, {offset: number, autoplay: boolean, abort: () => void}>}
 */
const pendingSwitches = new WeakMap();

/**
 * Fallback path: loads `url`, seeks to `offset` seconds once metadata is known (setting currentTime earlier is
 * ignored by some engines), then plays when `autoplay` is set. Resolves like {@link playMedia}, with "" when not
 * asked to play. A newer switchSource, cancelSwitch or detachMedia on the element resolves this one with
 * "AbortError" and drops its seek and play.
 * @param {HTMLMediaElement} media
 * @param {string} url
 * @param {number} offset
 * @param {boolean} autoplay
 * @returns {Promise<string>}
 */
export function switchSource(media, url, offset, autoplay) {
  cancelSwitch(media);
  return new Promise((resolve) => {
    const done = () => {
      media.removeEventListener('loadedmetadata', ready);
      media.removeEventListener('error', failed);
      if (pendingSwitches.get(media) === entry) pendingSwitches.delete(media);
    };
    const ready = () => {
      done();
      if (entry.offset > 0) media.currentTime = entry.offset;
      resolve(entry.autoplay ? playMedia(media) : '');
    };
    const failed = () => {
      done();
      resolve('MediaError');
    };
    const entry = {
      offset,
      autoplay,
      abort: () => {
        done();
        resolve('AbortError');
      },
    };
    pendingSwitches.set(media, entry);
    media.addEventListener('loadedmetadata', ready);
    media.addEventListener('error', failed);
    media.src = url;
  });
}

/**
 * Abandons the element's pending source switch, if any.
 * @param {HTMLMediaElement} media
 */
export function cancelSwitch(media) {
  const entry = pendingSwitches.get(media);
  if (entry) entry.abort();
}

/**
 * Offset in seconds a pending source switch will seek to, or NaN when none is pending (the element's currentTime is
 * then the playhead).
 * @param {HTMLMediaElement} media
 * @returns {number}
 */
export function pendingOffset(media) {
  const entry = pendingSwitches.get(media);
  return entry ? entry.offset : NaN;
}

/**
 * Moves the seek of a pending source switch to `offset` seconds. False when no switch is pending.
 * @param {HTMLMediaElement} media
 * @param {number} offset
 * @returns {boolean}
 */
export function retargetSwitch(media, offset) {
  const entry = pendingSwitches.get(media);
  if (!entry) return false;
  entry.offset = offset;
  return true;
}

/**
 * Whether a pending source switch starts playback when it loads (a pause while it loads turns that off).
 * @param {HTMLMediaElement} media
 * @param {boolean} autoplay
 */
export function setSwitchAutoplay(media, autoplay) {
  const entry = pendingSwitches.get(media);
  if (entry) entry.autoplay = autoplay;
}

/**
 * Releases the element's current source without removing the element.
 * @param {HTMLMediaElement} media
 */
export function detachMedia(media) {
  cancelSwitch(media);
  media.removeAttribute('src');
  media.load();
}

/**
 * @param {Uint8Array} bytes
 * @param {string} mime
 * @returns {string}
 */
export function blobUrl(bytes, mime) {
  return URL.createObjectURL(new Blob([bytes], { type: mime }));
}

/** @param {string} url */
export function revokeUrl(url) {
  URL.revokeObjectURL(url);
}

/**
 * @param {number} ms
 * @returns {Promise<void>}
 */
export function delay(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

/**
 * The `name` of a thrown value (DOMException, Error), or "" when it has none.
 * @param {unknown} err
 * @returns {string}
 */
export function errorName(err) {
  return err && typeof err === 'object' && 'name' in err ? String(err.name) : '';
}

/**
 * A readable message for a thrown value.
 * @param {unknown} err
 * @returns {string}
 */
export function errorMessage(err) {
  if (err && typeof err === 'object' && 'message' in err && err.message) return String(err.message);
  return String(err);
}

/**
 * A readable message for the element's current MediaError, or "" when there is none.
 * @param {HTMLMediaElement} media
 * @returns {string}
 */
export function mediaErrorMessage(media) {
  const err = media.error;
  if (!err) return '';
  const kinds = ['', 'aborted', 'network error', 'could not decode', 'format not supported'];
  const kind = kinds[err.code] || `error ${err.code}`;
  return err.message ? `${kind}: ${err.message}` : kind;
}
