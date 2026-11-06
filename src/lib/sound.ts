// Optional sounds (Settings → Look → Sounds, off by default). Synthesised on
// the fly so there are no audio files: a soft CRT power-on hum and a tiny
// tactile click. Always quiet.

let ctx: AudioContext | null = null;
function ac() {
  try {
    ctx ??= new AudioContext();
    if (ctx.state === "suspended") ctx.resume();
    return ctx;
  } catch {
    return null;
  }
}

/** CRT power-on: a low hum with a faint high whine, fading in and out. */
export function hum() {
  const c = ac();
  if (!c) return;
  const t = c.currentTime;
  const out = c.createGain();
  out.gain.setValueAtTime(0, t);
  out.gain.linearRampToValueAtTime(0.05, t + 0.12);
  out.gain.exponentialRampToValueAtTime(0.0001, t + 1.6);
  out.connect(c.destination);
  for (const [f, g] of [
    [60, 1],
    [120, 0.45],
    [15700, 0.04],
  ] as const) {
    const o = c.createOscillator();
    const gn = c.createGain();
    o.frequency.value = f;
    o.type = f > 1000 ? "sine" : "triangle";
    gn.gain.value = g;
    o.connect(gn).connect(out);
    o.start(t);
    o.stop(t + 1.7);
  }
}

/** A soft click for focus moves. */
export function click() {
  const c = ac();
  if (!c) return;
  const t = c.currentTime;
  const o = c.createOscillator();
  const g = c.createGain();
  o.type = "sine";
  o.frequency.setValueAtTime(1800, t);
  o.frequency.exponentialRampToValueAtTime(600, t + 0.03);
  g.gain.setValueAtTime(0.025, t);
  g.gain.exponentialRampToValueAtTime(0.0001, t + 0.05);
  o.connect(g).connect(c.destination);
  o.start(t);
  o.stop(t + 0.06);
}
