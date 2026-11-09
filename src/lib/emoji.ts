// Emoji that look the same for everyone. iPhone (and Mac) draw Apple's own;
// everywhere else they're Microsoft's Fluent 3D emoji (MIT licence,
// github.com/microsoft/fluentui-emoji), the glossy 3D set closest to iOS.
import map from "./fluent-map.json";
import { app } from "./stores/app.svelte";

const CDN = "https://cdn.jsdelivr.net/gh/microsoft/fluentui-emoji@main/assets/";
const M = map as Record<string, string>;

/** Apple's own emoji are already on screen: leave them be */
export const nativeEmoji = () => app.platform === "ios" || app.platform === "macos";

const code = (g: string) => [...g].map((c) => c.codePointAt(0)!.toString(16)).join("-");

/** the Fluent 3D picture for one emoji, or null (unknown: draw it as text) */
export function emojiUrl(g: string): string | null {
  if (nativeEmoji()) return null;
  const c = code(g);
  const rel = M[c] ?? M[c.replace(/-fe0f/g, "")] ?? M[`${c}-fe0f`];
  return rel ? CDN + rel.split("/").map(encodeURIComponent).join("/") : null;
}

const seg = typeof Intl !== "undefined" && (Intl as any).Segmenter ? new (Intl as any).Segmenter(undefined, { granularity: "grapheme" }) : null;
const PICT = /\p{Extended_Pictographic}|\p{Regional_Indicator}/u;

/** text split into plain runs and emoji (with their picture, if there is one) */
export function emojiParts(text: string): { t: string; src?: string }[] {
  if (!text || nativeEmoji() || !PICT.test(text)) return [{ t: text }];
  const out: { t: string; src?: string }[] = [];
  const graphemes: string[] = seg ? [...seg.segment(text)].map((s: any) => s.segment) : [...text];
  let run = "";
  for (const g of graphemes) {
    const src = PICT.test(g) ? emojiUrl(g) : null;
    if (src) {
      if (run) out.push({ t: run }), (run = "");
      out.push({ t: g, src });
    } else run += g;
  }
  if (run) out.push({ t: run });
  return out;
}
