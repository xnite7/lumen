// The title logo's own colour (its liveliest pixel), for the heart and the
// rating on a title's page. Falls back to null (the page keeps its pink).
const cache = new Map<string, string | null>();

export async function tintOf(url?: string | null): Promise<string | null> {
  if (!url) return null;
  if (cache.has(url)) return cache.get(url)!;
  const out = await new Promise<string | null>((done) => {
    const im = new Image();
    im.crossOrigin = "anonymous";
    im.onload = () => {
      try {
        const c = document.createElement("canvas");
        c.width = c.height = 32;
        const g = c.getContext("2d", { willReadFrequently: true });
        if (!g) return done(null);
        g.drawImage(im, 0, 0, 32, 32);
        const d = g.getImageData(0, 0, 32, 32).data;
        let best: [number, number, number, number] | null = null;
        for (let i = 0; i < d.length; i += 4) {
          if (d[i + 3] < 160) continue;
          const [r, gg, b] = [d[i] / 255, d[i + 1] / 255, d[i + 2] / 255];
          const mx = Math.max(r, gg, b), mn = Math.min(r, gg, b);
          const l = (mx + mn) / 2;
          const s = mx === mn ? 0 : (mx - mn) / (1 - Math.abs(2 * l - 1));
          const score = s * (1 - Math.abs(l - 0.55) * 1.6);
          if (!best || score > best[3]) best = [d[i], d[i + 1], d[i + 2], score];
        }
        done(best && best[3] > 0.18 ? `rgb(${best[0]} ${best[1]} ${best[2]})` : null);
      } catch {
        done(null);
      }
    };
    im.onerror = () => done(null);
    im.src = url;
  });
  cache.set(url, out);
  return out;
}

/// a small flag for a title that isn't in English (from its original language)
const FLAG: Record<string, string> = {
  ja: "jp", ko: "kr", zh: "cn", cn: "cn", hi: "in", ta: "in", te: "in", ml: "in", kn: "in", bn: "in", mr: "in", pa: "in",
  es: "es", fr: "fr", de: "de", it: "it", pt: "br", ru: "ru", tr: "tr", ar: "ae", sv: "se", da: "dk", no: "no", nb: "no",
  fi: "fi", pl: "pl", nl: "nl", th: "th", id: "id", ms: "my", tl: "ph", vi: "vn", fa: "ir", he: "il", el: "gr", cs: "cz",
  hu: "hu", ro: "ro", uk: "ua", is: "is", et: "ee", sr: "rs", hr: "hr", ur: "pk",
};
export function flagFor(lang?: string | null): string | null {
  const cc = lang ? FLAG[lang] : undefined;
  return cc ? `https://flagcdn.com/w40/${cc}.png` : null;
}
