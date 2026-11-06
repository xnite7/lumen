// One look: midnite. What's adjustable is how much of it you get -- neon
// intensity, grain, the artwork glow, motion -- plus the room getting cozier
// as the night goes on.

import { app } from "./app.svelte";
import { installSprings } from "../motion";

const NEON_K = { soft: 0.6, lit: 1, loud: 1.45 } as const;

export function applyCurrent() {
  const g = app.settings.general;
  const root = document.documentElement;
  root.style.setProperty("--grain-opacity", g.grain ? "0.03" : "0");
  root.style.setProperty("--ambient-strength", g.ambientGlow ? "0.3" : "0");
  root.style.setProperty("--neon-k", String(NEON_K[g.neon as keyof typeof NEON_K] ?? 1));
  root.dataset.neon = g.neon;
  root.classList.toggle("refraction", g.refraction);
  installSprings(g.motionScale);
  tickNight();
}

// ------------------------------------------------------------- night

/** 0 before 10 PM, rising to 1 by 4 AM, back to 0 by 7 AM. */
export function nightness(d = new Date()): number {
  const h = d.getHours() + d.getMinutes() / 60;
  if (h >= 22) return ((h - 22) / 6) * 0.5; // 10 PM..midnight: 0..0.17
  if (h < 4) return 0.33 + (h / 4) * 0.67; // midnight..4 AM: .33..1
  if (h < 7) return 1 - (h - 4) / 3; // dawn fades it out
  return 0;
}

let timer = 0;
function tickNight() {
  const on = app.settings.general.timeAware;
  document.documentElement.style.setProperty("--night", on ? nightness().toFixed(3) : "0");
  clearTimeout(timer);
  timer = window.setTimeout(tickNight, 60_000);
}

/** The greeting the room gives you right now (lowercase, deadpan). */
export function greeting(d = new Date()): string {
  const h = d.getHours();
  if (h >= 1 && h < 5) return "still up? same.";
  if (h >= 5 && h < 7) return "the birds are awake. are you?";
  if (h >= 7 && h < 12) return "morning. coffee's on.";
  if (h >= 12 && h < 17) return "afternoon on the couch.";
  if (h >= 17 && h < 22) return "evening. lights down.";
  return "it's late. perfect.";
}
