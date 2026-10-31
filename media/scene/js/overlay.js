// DOM overlays composited over the canvas: title cards and the end card. Opacity is computed from the frame time,
// so overlays are deterministic. Items: {start, end, type: 'title'|'endcard'|'caption', text, sub, position}.
const fade = (t, a, b, f = 0.5) => Math.max(0, Math.min(1, (t - a) / f, (b - t) / f));

export class Overlay {
  constructor(el) { this.el = el; this.items = []; }
  load(items, W, H) {
    this.el.innerHTML = '';
    this.el.style.setProperty('--u', (Math.min(W, H) / 1080).toString());
    this.items = items.map((it) => {
      const d = document.createElement('div');
      d.className = `ov ov--${it.type} ov--${it.position || 'center'}`;
      if (it.type === 'endcard') {
        d.innerHTML = `<img class="ov__logo" src="${it.logo || '/brand/wordmark/streams-reader-stacked-on-dark.svg'}" alt="">` +
          (it.text ? `<div class="ov__tag">${it.text}</div>` : '') + (it.sub ? `<div class="ov__url">${it.sub}</div>` : '');
      } else {
        d.innerHTML = `<div class="ov__text">${it.text || ''}</div>` + (it.sub ? `<div class="ov__sub">${it.sub}</div>` : '');
      }
      d.style.opacity = 0;
      this.el.appendChild(d);
      return { it, d };
    });
  }
  update(t) {
    for (const { it, d } of this.items) {
      const o = fade(t, it.start, it.end, it.fade ?? 0.5);
      d.style.opacity = o.toFixed(3);
      const rise = (1 - Math.min(1, (t - it.start) / 0.9));
      d.style.transform = `translateY(${(Math.max(0, rise) * rise * 18).toFixed(2)}px)`;
    }
  }
}
