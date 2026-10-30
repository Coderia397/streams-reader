// @ts-check
/**
 * Media Session glue for the nolabs player (lock screen, notification, headset buttons). Every call is a no-op when
 * the API is missing, and actions the browser does not know are skipped instead of throwing, which the web-sys
 * bindings cannot express.
 */

/** @returns {MediaSession | null} */
function session() {
  return typeof navigator !== 'undefined' && 'mediaSession' in navigator ? navigator.mediaSession : null;
}

/**
 * @param {string} title
 * @param {string} artist
 * @param {string} album
 * @param {string | undefined} artworkUrl
 */
export function setMetadata(title, artist, album, artworkUrl) {
  const ms = session();
  if (!ms || typeof MediaMetadata === 'undefined') return;
  const artwork = artworkUrl ? [{ src: artworkUrl }] : [];
  ms.metadata = new MediaMetadata({ title, artist, album, artwork });
}

export function clearMetadata() {
  const ms = session();
  if (!ms) return;
  ms.metadata = null;
  ms.playbackState = 'none';
  try {
    ms.setPositionState();
  } catch {
    // Older engines reject the no-argument form.
  }
}

/** @param {'none' | 'paused' | 'playing'} state */
export function setPlaybackState(state) {
  const ms = session();
  if (ms) ms.playbackState = state;
}

/**
 * Position inside the current segment, so the notification's scrubber covers one segment at a time.
 * @param {number} duration seconds, > 0
 * @param {number} position seconds, 0..=duration
 * @param {number} rate
 */
export function setPositionState(duration, position, rate) {
  const ms = session();
  if (!ms || !('setPositionState' in ms)) return;
  if (!(duration > 0) || !(rate > 0)) return;
  try {
    ms.setPositionState({ duration, position: Math.min(Math.max(position, 0), duration), playbackRate: rate });
  } catch {
    // Values out of range for this engine; the notification keeps its previous position.
  }
}

/**
 * Installs a handler for one action, or removes it when `handler` is null. The handler receives the number the
 * action carries: `seekTime` for seekto, `seekOffset` for seekbackward/seekforward, NaN when absent.
 * @param {MediaSessionAction} action
 * @param {((value: number) => void) | null} handler
 * @returns {boolean} whether the browser accepted the action
 */
export function setActionHandler(action, handler) {
  const ms = session();
  if (!ms) return false;
  try {
    ms.setActionHandler(
      action,
      handler
        ? (details) => {
            const value = action === 'seekto' ? details.seekTime : details.seekOffset;
            handler(typeof value === 'number' ? value : NaN);
          }
        : null,
    );
    return true;
  } catch {
    return false;
  }
}
