export function time(sec: number) {
  if (!isFinite(sec) || sec < 0) sec = 0;
  const h = Math.floor(sec / 3600);
  const m = Math.floor((sec % 3600) / 60);
  const s = Math.floor(sec % 60);
  return h ? `${h}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}` : `${m}:${String(s).padStart(2, "0")}`;
}

export function remaining(pos: number, dur: number) {
  const left = Math.max(0, dur - pos);
  const m = Math.round(left / 60);
  if (m >= 60) return `${Math.floor(m / 60)}h ${m % 60}m left`;
  return `${m}m left`;
}

export function bytes(n?: number) {
  if (!n) return "";
  if (n >= 1e12) return (n / 1e12).toFixed(1) + " TB";
  if (n >= 1e9) return (n / 1e9).toFixed(1) + " GB";
  if (n >= 1e6) return Math.round(n / 1e6) + " MB";
  return Math.round(n / 1e3) + " KB";
}

export function runtime(r?: string | number) {
  if (r == null || r === "") return "";
  const m = typeof r === "number" ? r : parseInt(String(r), 10);
  if (!m || isNaN(m)) return String(r);
  return m >= 60 ? `${Math.floor(m / 60)}h ${m % 60}m` : `${m}m`;
}

export function year(m: { releaseInfo?: string; released?: string }) {
  return m.releaseInfo || (m.released ? m.released.slice(0, 4) : "");
}

export function episodeLabel(videoId?: string) {
  if (!videoId) return "";
  const p = videoId.split(":");
  if (p.length < 3) return "";
  return `S${p[p.length - 2]} E${p[p.length - 1]}`;
}

const LANGS: Record<string, string> = {
  eng: "English", en: "English", ara: "Arabic", ar: "Arabic", jpn: "Japanese", ja: "Japanese", spa: "Spanish", es: "Spanish",
  fre: "French", fra: "French", fr: "French", ger: "German", deu: "German", de: "German", ita: "Italian", it: "Italian",
  por: "Portuguese", pt: "Portuguese", pob: "Portuguese (BR)", rus: "Russian", ru: "Russian", kor: "Korean", ko: "Korean",
  chi: "Chinese", zho: "Chinese", zh: "Chinese", hin: "Hindi", hi: "Hindi", tur: "Turkish", tr: "Turkish", dut: "Dutch", nld: "Dutch",
  pol: "Polish", swe: "Swedish", nor: "Norwegian", dan: "Danish", fin: "Finnish", heb: "Hebrew", gre: "Greek", ell: "Greek",
  per: "Persian", fas: "Persian", urd: "Urdu", ind: "Indonesian", tha: "Thai", vie: "Vietnamese", und: "Unknown",
};
export function lang(code?: string) {
  if (!code) return "Unknown";
  return LANGS[code.toLowerCase()] ?? code.toUpperCase();
}
