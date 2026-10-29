// Subtitle text -> structured lines for the glass overlay.
// Ported from Enhanced Player: ASS tags stripped (top-alignment kept),
// SDH segments ("(door slams)", "[music]", "♪ lyrics ♪", "JOHN:") marked so
// they can be styled or hidden, and Arabic lines flagged for RTL.

const ASS_TAG = /\{\\[^}]*\}/g;
const ASS_TOP = /\{[^}]*\\an[789](?!\d)[^}]*\}/i;
const ARABIC = /[؀-ۿݐ-ݿࢠ-ࣿﭐ-﷿ﹰ-﻿]/;
const SDH_SEGMENT = /(\([^()\n]*\)|\[[^[\]\n]*\]|[♪♫][^♪♫\n]*[♪♫]?)/g;
const SDH_SPEAKER = /^[A-Z][A-Z0-9 .'’#-]{1,24}:(?=\s|$)/;

export interface Segment {
  text: string;
  sdh?: boolean;
  speaker?: boolean;
  italic?: boolean;
}
export interface Line {
  segments: Segment[];
  rtl: boolean;
}
export interface Cue {
  lines: Line[];
  top: boolean;
  key: string;
}

function split(line: string): Segment[] {
  const out: Segment[] = [];
  let rest = line;
  const sp = rest.match(SDH_SPEAKER);
  if (sp) {
    out.push({ text: sp[0], speaker: true });
    rest = rest.slice(sp[0].length);
  }
  let last = 0;
  SDH_SEGMENT.lastIndex = 0;
  let m: RegExpExecArray | null;
  while ((m = SDH_SEGMENT.exec(rest)) !== null) {
    if (m.index > last) out.push({ text: rest.slice(last, m.index) });
    out.push({ text: m[0], sdh: true });
    last = m.index + m[0].length;
  }
  if (last < rest.length) out.push({ text: rest.slice(last) });
  return out;
}

export function parseCue(raw: string, opts: { hideSdh: boolean }): Cue | null {
  if (!raw || !raw.trim()) return null;
  const top = ASS_TOP.test(raw);
  // <i>..</i> from SRT and {\i1} from ASS both become italic segments.
  const italicAll = /^\s*(<i>|\{\\i1\})/.test(raw);
  const text = raw
    .replace(ASS_TAG, "")
    .replace(/\\N/g, "\n")
    .replace(/<\/?(i|b|u|font)[^>]*>/gi, "")
    .replace(/\r/g, "");
  const lines: Line[] = [];
  for (const l of text.split("\n")) {
    const t = l.trim();
    if (!t) continue;
    let segs = split(t);
    if (opts.hideSdh) {
      segs = segs.filter((s) => !s.sdh && !s.speaker).map((s) => ({ ...s, text: s.text }));
      if (!segs.some((s) => s.text.trim())) continue;
    }
    if (italicAll) segs = segs.map((s) => ({ ...s, italic: true }));
    lines.push({ segments: segs, rtl: ARABIC.test(t) });
  }
  if (!lines.length) return null;
  return { lines, top, key: text };
}
