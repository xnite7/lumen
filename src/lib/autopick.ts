// Picks the stream for you: the best one your connection can keep up with,
// plus the order to fall back through if it won't start.

import { api, type Stream } from "./api";

/** Rough bitrate a resolution needs when the file size isn't known (Mbps). */
const NEED: Record<string, number> = { "4K": 40, "1080p": 12, "720p": 5, SD: 2, Unknown: 8 };

function runtimeSecs(runtime?: string | number): number {
  if (typeof runtime === "number") return runtime * 60;
  const m = String(runtime ?? "").match(/(\d+)\s*h(?:\s*(\d+))?|(\d+)\s*min/i);
  if (!m) return 0;
  if (m[3]) return +m[3] * 60;
  return (+m[1] * 60 + +(m[2] ?? 0)) * 60;
}

/** Megabits per second this stream needs to play without stalling. */
export function needMbps(st: Stream, runtime?: string | number): number {
  const p = st._couch;
  const secs = runtimeSecs(runtime) || 2 * 3600;
  if (p?.sizeBytes) return (p.sizeBytes * 8) / secs / 1e6;
  return NEED[p?.res ?? "Unknown"] ?? 8;
}

export async function connectionMbps(fresh = false): Promise<number> {
  try {
    return await api.netSpeed(fresh);
  } catch {
    return 25; // unknown: assume a decent home connection
  }
}

/** iPhone: Apple's player (MP4/HLS) can float in picture-in-picture when you
 *  leave the app; MKV files need couch's own player, which can't. */
function pipFriendly(s: Stream): boolean {
  const f = String((s as any).behaviorHints?.filename ?? (s as any).url ?? "").toLowerCase();
  return /\.(mp4|m4v|mov)(\?|$)|\.m3u8/.test(f);
}

/**
 * Order streams for autoplay: playable-now first (cached debrid / direct),
 * then those that fit the connection with headroom, best first; the rest
 * after, smallest first, so a fallback is more likely to start.
 */
export function rank(streams: Stream[], mbps: number, runtime?: string | number, ios = false): Stream[] {
  const ok = streams.filter((s) => s._couch?.kind !== "external" && s._couch?.kind !== "youtube" && !(s._couch?.threeD && !s._couch?.threeDLayout));
  const budget = mbps * 0.7;
  const instant = (s: Stream) => s._couch?.cached !== false && (s._couch?.kind === "debrid" || s._couch?.kind === "http");
  const fits = ok.filter((s) => needMbps(s, runtime) <= budget && !s._couch?.threeD);
  const rest = ok.filter((s) => !fits.includes(s));
  // on iPhone, at the same resolution, a picture-in-picture friendly file wins
  const pip = (s: Stream) => (ios && pipFriendly(s) ? 1 : 0);
  const byScore = (a: Stream, b: Stream) =>
    (instant(b) ? 1 : 0) - (instant(a) ? 1 : 0) ||
    (b._couch?.resRank ?? 0) - (a._couch?.resRank ?? 0) ||
    pip(b) - pip(a) ||
    (b._couch?.score ?? 0) - (a._couch?.score ?? 0);
  fits.sort(byScore);
  rest.sort((a, b) => (instant(b) ? 1 : 0) - (instant(a) ? 1 : 0) || needMbps(a, runtime) - needMbps(b, runtime));
  return [...fits, ...rest];
}
