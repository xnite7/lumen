// People's names in their Discord display-name style (font + colour), looked
// up a few at a time from the crew server, the same fonts as the PC app.
import { api } from "$lib/api";

export type NameStyle = { font?: string; color?: string } | null;

const FONTS: Record<number, string> = {
  1: "Bangers", 2: "BioRhyme", 3: "Sakura", 4: "Jellybean", 6: "Modern", 7: "Medieval", 8: "8Bit",
  10: "Vampyre", 12: "Tempo", 13: "Monkey Bars", 14: "Mainframe", 15: "Headbang", 16: "Journal",
};

const known = $state<Record<string, NameStyle>>({});
let want = new Set<string>();
let timer = 0;

export function nameStyle(id?: string): NameStyle {
  if (!id) return null;
  if (id in known) return known[id];
  want.add(id);
  if (!timer) {
    timer = window.setTimeout(async () => {
      timer = 0;
      const ids = [...want];
      want = new Set();
      try {
        const r = await api.social<any>("GET", `/v1/looks?ids=${ids.join(",")}`);
        for (const i of ids) {
          const ns = r?.looks?.[i];
          const f = ns?.font as number | undefined;
          // newer styles (the GTA one is Pricedown, not shipped): a heavy face
          const font = f && f !== 11 ? FONTS[f] ?? "Bangers" : undefined;
          known[i] = font || ns?.colors?.[0] ? { font, color: ns?.colors?.[0] } : null;
        }
      } catch {
        for (const i of ids) known[i] = null;
      }
    }, 150);
  }
  return null;
}

export function nameCss(id?: string): string {
  const s = nameStyle(id);
  if (!s) return "";
  return `${s.font ? `font-family:"${s.font}",Satoshi;font-weight:400;` : ""}${s.color ? `color:${s.color};` : ""}`;
}
