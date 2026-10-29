// The light source. Whatever artwork is "in focus" (the hero slide, the
// detail page, the playing title) drives the accent colour, the glow behind
// the page, and -- if you allow it -- the Hue lights in your room.

import { api } from "../api";
import { app } from "./app.svelte";

export const ambient = $state({
  art: "" as string,
  colors: ["#ffc22e", "#77b9e4", "#e4736a"] as string[],
  dark: true,
});

const cache = new Map<string, { colors: string[]; dark: boolean }>();
let seq = 0;
let moodTimer: ReturnType<typeof setTimeout> | undefined;

const hex = (c: number[]) => "#" + c.map((x) => Math.max(0, Math.min(255, Math.round(x))).toString(16).padStart(2, "0")).join("");

/** Lift very dark colours so the accent still reads on dark glass. */
function usable(c: number[]): number[] {
  const l = 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
  if (l >= 90) return c;
  const k = 90 / Math.max(l, 1);
  return c.map((x) => Math.min(255, x * k));
}

function inkFor(c: string) {
  const n = parseInt(c.slice(1), 16);
  const l = 0.2126 * ((n >> 16) & 255) + 0.7152 * ((n >> 8) & 255) + 0.0722 * (n & 255);
  return l > 150 ? "#14110a" : "#fbf8f2";
}

function applyColors(colors: string[]) {
  const root = document.documentElement.style;
  root.setProperty("--ambient-1", colors[0]);
  root.setProperty("--ambient-2", colors[1] ?? colors[0]);
  root.setProperty("--ambient-3", colors[2] ?? colors[1] ?? colors[0]);
  if (app.settings.general.accentFromArtwork) {
    root.setProperty("--accent", colors[0]);
    root.setProperty("--accent-ink", inkFor(colors[0]));
  } else {
    root.removeProperty("--accent");
    root.removeProperty("--accent-ink");
  }
}

export async function setAmbient(url?: string | null) {
  if (!url || url === ambient.art) return;
  ambient.art = url;
  const my = ++seq;
  let p = cache.get(url);
  if (!p) {
    try {
      const r = await api.palette(url);
      p = { colors: (r.colors.length ? r.colors : [r.average]).slice(0, 3).map((c) => hex(usable(c))), dark: r.dark };
      cache.set(url, p);
    } catch {
      return;
    }
  }
  if (my !== seq) return;
  ambient.colors = p.colors;
  ambient.dark = p.dark;
  applyColors(p.colors);
  // The room follows slowly; don't hammer the bridge while scrubbing a carousel.
  clearTimeout(moodTimer);
  if (app.settings.hue.enabled && app.settings.hue.artworkMood && !app.playing) {
    moodTimer = setTimeout(() => api.hueMood(url).catch(() => {}), 2200);
  }
}
