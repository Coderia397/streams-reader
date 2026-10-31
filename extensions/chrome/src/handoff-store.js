// @ts-check
/**
 * Pending handoffs, kept in chrome.storage.session (memory only, cleared when the browser closes, and readable only
 * by the extension's own trusted contexts, not by content scripts). Each entry is bound to the app tab opened for it.
 *
 * Handing out is two-phase so a reply lost on the way (the app timed out, the tab reloaded) is not a lost article:
 * claim() leases the entry to the asking tab for `leaseMs`, and ack() from that tab deletes it. The tab holding the
 * lease may claim the entry again at once: the app acks every payload it receives, so a second ask from the same tab
 * means the first reply never arrived. Other tabs can claim it only after the lease ran out without an ack.
 *
 * Entries expire `ttlMs` after they are stored. Expired entries are removed whenever the store is read, and by a
 * sweep that `scheduleSweep` arms for the earliest expiry (an alarm in the extension), so an entry nobody collects
 * does not stay in memory until the next use of the extension.
 *
 * The storage area, the clock and the scheduler are injected so tests/unit.js can run this without the extension
 * APIs.
 */

const PREFIX = "handoff:";

/**
 * @typedef {import("./payload.js").Payload} Payload
 * @typedef {{ payload: Payload, tabId: number, expiresAt: number, leasedTo?: number, leasedUntil?: number }} Entry
 * @typedef {{
 *   get(keys: null | string | string[]): Promise<Record<string, unknown>>,
 *   set(items: Record<string, unknown>): Promise<void>,
 *   remove(keys: string | string[]): Promise<void>,
 * }} Area
 */

/**
 * @param {{
 *   area: Area,
 *   ttlMs: number,
 *   leaseMs: number,
 *   now?: () => number,
 *   tabExists: (tabId: number) => Promise<boolean>,
 *   scheduleSweep?: (when: number | null) => Promise<void> | void,
 * }} deps  `scheduleSweep(when)` asks for sweep() to run at `when` (ms since the epoch), replacing any earlier
 *          request; null cancels it.
 */
export function createStore({ area, ttlMs, leaseMs, now = Date.now, tabExists, scheduleSweep = () => {} }) {
  /** Slack after an expiry before the sweep runs, so the entry is past its time when the sweep reads the clock. */
  const SWEEP_SLACK_MS = 1000;
  /** @returns {Promise<Array<[string, Entry]>>} live entries, oldest first; expired ones are removed. */
  async function live() {
    const all = await area.get(null);
    const t = now();
    /** @type {Array<[string, Entry]>} */
    const keep = [];
    /** @type {string[]} */
    const stale = [];
    for (const [k, v] of Object.entries(all)) {
      if (!k.startsWith(PREFIX)) continue;
      const e = /** @type {Entry} */ (v);
      if (!e || typeof e.expiresAt !== "number" || e.expiresAt <= t) stale.push(k);
      else keep.push([k, e]);
    }
    if (stale.length) await area.remove(stale);
    keep.sort((a, b) => a[1].payload.createdAt - b[1].payload.createdAt);
    return keep;
  }

  /**
   * Arms the sweep for the earliest expiry among `entries`, or cancels it when there are none. A failing scheduler
   * only delays removal until the next read, so its errors are ignored.
   * @param {Array<[string, Entry]>} entries
   */
  async function arm(entries) {
    const next = entries.reduce((m, [, e]) => Math.min(m, e.expiresAt), Infinity);
    try {
      await scheduleSweep(next === Infinity ? null : next + SWEEP_SLACK_MS);
    } catch {
      // Removal falls back to the next put, claim or sweep.
    }
  }

  return {
    /**
     * Stores a payload for the app tab `tabId`. When the area is full, the oldest pending handoffs are dropped
     * until it fits.
     * @param {Payload} payload
     * @param {number} tabId
     */
    async put(payload, tabId) {
      const key = PREFIX + payload.id;
      /** @type {Entry} */
      const entry = { payload, tabId, expiresAt: now() + ttlMs };
      const others = await live();
      for (;;) {
        try {
          await area.set({ [key]: entry });
          break;
        } catch (e) {
          const oldest = others.shift();
          if (!oldest) throw e;
          await area.remove(oldest[0]);
        }
      }
      await arm([...others, [key, entry]]);
    },

    /**
     * Leases the newest free handoff meant for `tabId` to it. A handoff is free for `tabId` when it is not leased,
     * its lease ran out, or it is leased to `tabId` itself (the reply was lost and the tab asks again). A handoff
     * whose app tab has since closed may be claimed by any app tab, so a sign-in in a new tab still receives it.
     * @param {number} tabId
     * @returns {Promise<Payload | null>}
     */
    async claim(tabId) {
      const t = now();
      const free = (await live())
        .filter(([, e]) => !(e.leasedUntil && e.leasedUntil > t && e.leasedTo !== tabId))
        .reverse();
      let pick = free.find(([, e]) => e.tabId === tabId);
      if (!pick) {
        for (const candidate of free) {
          if (!(await tabExists(candidate[1].tabId))) {
            pick = candidate;
            break;
          }
        }
      }
      if (!pick) return null;
      const [key, entry] = pick;
      await area.set({ [key]: { ...entry, leasedTo: tabId, leasedUntil: t + leaseMs } });
      return entry.payload;
    },

    /**
     * Deletes the handoff `id` once the tab it was leased to confirms it has it.
     * @param {number} tabId
     * @param {string} id
     * @returns {Promise<boolean>}
     */
    async ack(tabId, id) {
      const key = PREFIX + id;
      const entry = /** @type {Entry | undefined} */ ((await area.get(key))[key]);
      if (!entry || entry.leasedTo !== tabId) return false;
      await area.remove(key);
      return true;
    },

    /** Drops expired entries and arms the next sweep. */
    async sweep() {
      await arm(await live());
    },
  };
}

/**
 * Runs async functions one at a time, in call order. Used so the app tab cannot claim before its handoff is stored.
 */
export function createMutex() {
  /** @type {Promise<unknown>} */
  let tail = Promise.resolve();
  return {
    /**
     * @template T
     * @param {() => Promise<T>} fn
     * @returns {Promise<T>}
     */
    run(fn) {
      const result = tail.then(() => fn());
      tail = result.catch(() => {});
      return result;
    },
  };
}
