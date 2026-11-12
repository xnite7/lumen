// Title logos often carry empty transparent space under the lettering and to
// its left, so they float above the line beneath them and sit off the left
// edge. This measures both once per image and pulls the logo back by them.

type Blank = { bottom: number; left: number };
const none: Blank = { bottom: 0, left: 0 };
const cache = new Map<string, Promise<Blank>>();

function blank(src: string): Promise<Blank> {
  let p = cache.get(src);
  if (p) return p;
  p = new Promise<Blank>((done) => {
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
        let first = 0;
        scanx: for (; first < w; first++) {
          for (let y = 0; y < h; y++) if (d[(y * w + first) * 4 + 3] > 12) break scanx;
        }
        done(last < 0 ? none : { bottom: (h - 1 - last) / h, left: first >= w ? 0 : first / w });
      } catch {
        done(none); // no CORS on this host: leave it as it is
      }
    };
    im.onerror = () => done(none);
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
    const b = await blank(src);
    if (gone || (el.currentSrc || el.src) !== src) return;
    const r = el.getBoundingClientRect();
    el.style.setProperty("--blank", String(b.bottom));
    el.style.marginBottom = b.bottom > 0.02 ? `${-b.bottom * r.height}px` : "";
    // object-fit: contain keeps the picture's shape, so its left blank is a share of the drawn width
    el.style.marginLeft = b.left > 0.02 ? `${-b.left * r.width}px` : "";
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
