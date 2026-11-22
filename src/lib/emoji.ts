type EmojiPart = { t: string; emoji?: boolean };
const seg = typeof Intl !== "undefined" && (Intl as any).Segmenter ? new (Intl as any).Segmenter(undefined, { granularity: "grapheme" }) : null;
const PICT = /\p{Extended_Pictographic}|\p{Regional_Indicator}/u;

/** Split emoji into runs so the system emoji font can be selected explicitly. */
export function emojiParts(text: string): EmojiPart[] {
  if (!text || !PICT.test(text)) return [{ t: text }];
  const out: EmojiPart[] = [];
  const graphemes: string[] = seg ? [...seg.segment(text)].map((s: any) => s.segment) : [...text];
  let run = "";
  for (const g of graphemes) {
    if (PICT.test(g)) {
      if (run) out.push({ t: run }), (run = "");
      out.push({ t: g, emoji: true });
    } else run += g;
  }
  if (run) out.push({ t: run });
  return out;
}
