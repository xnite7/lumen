// Format badges, streaming-service logos and language flags from the
// Kingsize Fusion Badges pack (github.com/kingsizew/badges), loaded from the
// pack's own raw URLs the way AIOStreams / Nuvio use it.
//
// Only the top three tiers are shown (T1 gold, T2 blue, T3 green in the
// pack's Smart Tier List); fallbacks like SDR, HLG, 10bit, plain DD/DTS/AAC
// are left off.

const RAW = "https://raw.githubusercontent.com/kingsizew/badges/main/badge-images/";
const url = (p: string) => RAW + p;

export type Badge = { src: string; alt: string; tier: 1 | 2 | 3; kind: "res" | "visual" | "audio" | "imax" };

/**
 * Badges for what's playing. `c` = couch's parse of the stream name
 * (_couch: hdr, audio, res), `v` = what the video actually is (mpv),
 * `audioCodec` = the selected audio track's codec, when known.
 */
export function formatBadges(c: { hdr?: string[]; audio?: string[]; res?: string } | undefined, v: { height?: number; hdr?: boolean; gamma?: string }, audioCodec?: string): Badge[] {
  const out: Badge[] = [];
  const hdr = c?.hdr ?? [];
  const aud = c?.audio ?? [];
  const h = v.height || 0;

  // resolution: 4K > 1440p > 1080p
  const res = h >= 2000 || c?.res === "4K" ? "4k" : h >= 1400 ? "1440p" : h >= 1000 || c?.res === "1080p" ? "1080p" : null;
  if (res) out.push({ src: url(`resolution/${res}.png`), alt: res.toUpperCase(), tier: res === "4k" ? 1 : res === "1440p" ? 2 : 3, kind: "res" });

  // visual: one winner down the hierarchy (DV · HDR10+ … HDR)
  const dv = hdr.includes("DV");
  const hdr10p = hdr.includes("HDR10+");
  const hdr10 = hdr.includes("HDR10");
  const anyHdr = hdr.includes("HDR") || (v.hdr && v.gamma !== "hlg");
  const visual: [string, string, 1 | 2 | 3] | null = dv
    ? hdr10p
      ? ["dolby-vision-hdr10-plus", "Dolby Vision · HDR10+", 1]
      : hdr10
        ? ["dolby-vision-hdr10", "Dolby Vision · HDR10", 1]
        : anyHdr
          ? ["dolby-vision-hdr", "Dolby Vision · HDR", 1]
          : ["dolby-vision", "Dolby Vision", 1]
    : hdr10p
      ? ["hdr10-plus", "HDR10+", 1]
      : hdr10
        ? ["hdr10", "HDR10", 2]
        : anyHdr
          ? ["hdr", "HDR", 3]
          : null;
  if (visual) out.push({ src: url(`visual/${visual[0]}.png`), alt: visual[1], tier: visual[2], kind: "visual" });

  // audio: the Dolby winner and the DTS winner, FLAC as a T2
  const ac = (audioCodec ?? "").toLowerCase();
  const atmos = aud.includes("Atmos");
  const truehd = aud.includes("TrueHD") || ac === "truehd";
  const ddp = aud.includes("DD+") || ac === "eac3";
  const dtsx = aud.includes("DTS:X");
  const dtsma = aud.includes("DTS-HD MA");
  const dtshd = dtsma || aud.includes("DTS-HD") || ac === "dts-hd";
  const dolby: [string, string, 1 | 2 | 3] | null = atmos
    ? truehd
      ? ["dolby-atmos-truehd", "Dolby Atmos · TrueHD", 1]
      : ddp
        ? ["dolby-atmos-digital-plus", "Dolby Atmos · Digital Plus", 2]
        : ["dolby-atmos", "Dolby Atmos", 2]
    : truehd
      ? ["dolby-truehd", "Dolby TrueHD", 2]
      : ddp
        ? ["dolby-digital-plus", "Dolby Digital Plus", 3]
        : null;
  if (dolby) out.push({ src: url(`audio/${dolby[0]}.png`), alt: dolby[1], tier: dolby[2], kind: "audio" });
  const dts: [string, string, 1 | 2 | 3] | null = dtsx
    ? dtsma
      ? ["dts-x-hd-ma", "DTS:X · HD MA", 1]
      : dtshd
        ? ["dts-x-hd", "DTS:X · HD", 2]
        : ["dts-x", "DTS:X", 2]
    : dtsma
      ? ["dts-hd-ma", "DTS-HD MA", 2]
      : dtshd
        ? ["dts-hd", "DTS-HD", 3]
        : null;
  if (dts) out.push({ src: url(`audio/${dts[0]}.png`), alt: dts[1], tier: dts[2], kind: "audio" });
  if (!dolby && !dts && (aud.includes("FLAC") || ac === "flac")) out.push({ src: url("audio/flac.png"), alt: "FLAC", tier: 2, kind: "audio" });

  return out;
}

// ------------------------------------------------------------ services
/** TMDB watch-provider id → the pack's logo */
const SERVICE: Record<number, string> = {
  8: "netflix",
  1899: "hbo-max",
  384: "hbo-max",
  337: "disney-plus",
  9: "prime-video",
  119: "prime-video",
  350: "apple-tv-plus",
  15: "hulu",
  531: "paramount-plus",
  386: "peacock",
  283: "crunchyroll",
};
export const serviceLogo = (tmdbProvider: number): string | null => (SERVICE[tmdbProvider] ? url(`streaming/${SERVICE[tmdbProvider]}.png`) : null);

// ------------------------------------------------------------- flags
/** language code (ISO 639-1/2, as mpv and addons give them) → the pack's flag */
const LANG: Record<string, string> = {
  ar: "arabic", ara: "arabic",
  bn: "bengali", ben: "bengali",
  bg: "bulgarian", bul: "bulgarian",
  zh: "chinese", chi: "chinese", zho: "chinese", "zh-cn": "chinese", "zh-tw": "chinese", chs: "chinese", cht: "chinese",
  hr: "croatian", hrv: "croatian",
  cs: "czech", cze: "czech", ces: "czech",
  da: "danish", dan: "danish",
  nl: "dutch", dut: "dutch", nld: "dutch",
  en: "english", eng: "english", "en-us": "english", "en-gb": "english",
  et: "estonian", est: "estonian",
  fi: "finnish", fin: "finnish",
  fr: "french", fre: "french", fra: "french", "fr-ca": "french",
  de: "german", ger: "german", deu: "german",
  el: "greek", gre: "greek", ell: "greek",
  gu: "gujarati", guj: "gujarati",
  he: "hebrew", heb: "hebrew", iw: "hebrew",
  hi: "hindi", hin: "hindi",
  hu: "hungarian", hun: "hungarian",
  id: "indonesian", ind: "indonesian",
  it: "italian", ita: "italian",
  ja: "japanese", jpn: "japanese",
  kn: "kannada", kan: "kannada",
  ko: "korean", kor: "korean",
  "es-419": "latino", "es-la": "latino", "es-mx": "latino", lat: "latino",
  lv: "latvian", lav: "latvian",
  lt: "lithuanian", lit: "lithuanian",
  ms: "malay", may: "malay", msa: "malay",
  ml: "malayalam", mal: "malayalam",
  mr: "marathi", mar: "marathi",
  no: "norwegian", nor: "norwegian", nb: "norwegian", nob: "norwegian", nn: "norwegian",
  fa: "persian", per: "persian", fas: "persian",
  pl: "polish", pol: "polish",
  "pt-br": "portuguese-brazil", pob: "portuguese-brazil", "pt_br": "portuguese-brazil",
  pt: "portuguese", por: "portuguese", "pt-pt": "portuguese",
  pa: "punjabi", pan: "punjabi",
  ro: "romanian", rum: "romanian", ron: "romanian",
  ru: "russian", rus: "russian",
  sr: "serbian", srp: "serbian", scc: "serbian",
  sk: "slovak", slo: "slovak", slk: "slovak",
  sl: "slovenian", slv: "slovenian",
  es: "spanish", spa: "spanish", "es-es": "spanish",
  sv: "swedish", swe: "swedish",
  ta: "tamil", tam: "tamil",
  te: "telugu", tel: "telugu",
  th: "thai", tha: "thai",
  tr: "turkish", tur: "turkish",
  uk: "ukrainian", ukr: "ukrainian",
  vi: "vietnamese", vie: "vietnamese",
};
/** the flag for a track's language, or null (no flag in the pack) */
export function langFlag(code?: string | null): string | null {
  if (!code) return null;
  const k = code.toLowerCase().trim();
  const name = LANG[k] ?? LANG[k.split(/[-_]/)[0]];
  return name ? url(`language/${name}.png`) : null;
}
