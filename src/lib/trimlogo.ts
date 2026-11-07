// Title logos often carry empty transparent space under the lettering, so
// they float above the line beneath them. This measures that space once per
// image and pulls whatever follows up by the same amount.

const cache = new Map<string, Promise<number>>();

function blankBottom(src: string): Promise<number> {
  let p = cache.get(src);
  if (p) return p;
  p = new Promise<number>((done) => {
    const im = new Image();
    im.crossOrigin = "anonymous";
    im.onload = () => {
      try {
        const w = Math.min(240, im.naturalWidth || 240);
        const h = Math.max(1, Math.round((w * im.naturalHeight) / Math.max(1, im.naturalWidth)));
        const c = document.createElement("canvas");
        c.width = w;
        c.height = h;
        const g = c.getContext("2d", { willReadFrequently: true })!;
        g.drawImage(im, 0, 0, w, h);
        const d = g.getImageData(0, 0, w, h).data;
        let last = h - 1;
        scan: for (; last >= 0; last--) {
          for (let x = 0; x < w; x++) if (d[(last * w + x) * 4 + 3] > 12) break scan;
        }
        done(last < 0 ? 0 : (h - 1 - last) / h);
      } catch {
        done(0); // no CORS on this host: leave it as it is
      }
    };
    im.onerror = () => done(0);
    im.src = src;
  });
  cache.set(src, p);
  return p;
}

export function trimLogo(el: HTMLImageElement) {
  let gone = false;
  const apply = async () => {
    const src = el.currentSrc || el.src;
    if (!src) return;
    const f = await blankBottom(src);
    if (gone || (el.currentSrc || el.src) !== src) return;
    el.style.setProperty("--blank", String(f));
    el.style.marginBottom = f > 0.02 ? `calc(${-f} * ${el.getBoundingClientRect().height}px)` : "";
  };
  const ro = new ResizeObserver(apply);
  ro.observe(el);
  el.addEventListener("load", apply);
  if (el.complete) apply();
  return {
    destroy() {
      gone = true;
      ro.disconnect();
      el.removeEventListener("load", apply);
    },
  };
}
