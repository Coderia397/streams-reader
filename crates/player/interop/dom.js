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
 * Fallback path: loads `url`, seeks to `offset` seconds once metadata is known (setting currentTime earlier is
 * ignored by some engines), then plays when `autoplay` is set. Resolves like {@link playMedia}, with "" when not
 * asked to play.
 * @param {HTMLMediaElement} media
 * @param {string} url
 * @param {number} offset
 * @param {boolean} autoplay
 * @returns {Promise<string>}
 */
export function switchSource(media, url, offset, autoplay) {
  return new Promise((resolve) => {
    const ready = () => {
      media.removeEventListener('error', failed);
      if (offset > 0) media.currentTime = offset;
      resolve(autoplay ? playMedia(media) : '');
    };
    const failed = () => {
      media.removeEventListener('loadedmetadata', ready);
      resolve('MediaError');
    };
    media.addEventListener('loadedmetadata', ready, { once: true });
    media.addEventListener('error', failed, { once: true });
    media.src = url;
  });
}

/**
 * Releases the element's current source without removing the element.
 * @param {HTMLMediaElement} media
 */
export function detachMedia(media) {
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
