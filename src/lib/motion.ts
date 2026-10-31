// A real damped spring, solved once and expressed as CSS `linear()` so CSS
// transitions move exactly like iOS springs instead of approximating them
// with a cubic-bezier overshoot.

function solve(stiffness: number, damping: number, mass = 1, samples = 64) {
  const w0 = Math.sqrt(stiffness / mass);
  const zeta = damping / (2 * Math.sqrt(stiffness * mass));
  const wd = w0 * Math.sqrt(Math.max(0, 1 - zeta * zeta));
  const x = (t: number) =>
    zeta < 1
      ? 1 - Math.exp(-zeta * w0 * t) * (Math.cos(wd * t) + ((zeta * w0) / wd) * Math.sin(wd * t))
      : 1 - Math.exp(-w0 * t) * (1 + w0 * t);
  // settle time: when the envelope drops under 0.1%
  const settle = Math.min(4, -Math.log(0.001) / (zeta * w0));
  const pts: string[] = [];
  for (let i = 0; i <= samples; i++) {
    const t = (i / samples) * settle;
    pts.push(x(t).toFixed(4));
  }
  return { easing: `linear(${pts.join(", ")})`, duration: settle * 1000 };
}

export function installSprings(scale: number) {
  const root = document.documentElement.style;
  const supports = CSS.supports("transition-timing-function", "linear(0, 1)");
  root.setProperty("--motion", String(Math.max(0.01, scale)));
  document.documentElement.classList.toggle("no-motion", scale === 0);
  if (!supports) return;
  // Bouncy (buttons, sheets, popovers) and smooth (page moves).
  const bouncy = solve(260, 22);
  const smooth = solve(170, 26);
  root.setProperty("--spring", bouncy.easing);
  root.setProperty("--glide", smooth.easing);
}

/** Spring a number from a to b with rAF (for things CSS can't animate). */
export function springTo(from: number, to: number, onFrame: (v: number) => void, stiffness = 170, damping = 24) {
  let x = from;
  let v = 0;
  let last = performance.now();
  let raf = 0;
  const step = (now: number) => {
    const dt = Math.min(0.032, (now - last) / 1000);
    last = now;
    const f = -stiffness * (x - to) - damping * v;
    v += f * dt;
    x += v * dt;
    onFrame(x);
    if (Math.abs(v) > 0.001 || Math.abs(x - to) > 0.001) raf = requestAnimationFrame(step);
    else onFrame(to);
  };
  raf = requestAnimationFrame(step);
  return () => cancelAnimationFrame(raf);
}

/**
 * Drive a value with a physical spring that starts at the gesture's own
 * velocity (px/ms), so a release continues the finger's motion instead of
 * restarting it. Critically damped by default: no bounce, fastest settle.
 */
export function springGesture(opts: {
  from: number;
  to: number;
  velocity?: number;
  stiffness?: number;
  damping?: number;
  onUpdate: (v: number) => void;
  onDone?: () => void;
}) {
  const k = opts.stiffness ?? 420;
  const c = opts.damping ?? 2 * Math.sqrt(k);
  let x = opts.from;
  let v = (opts.velocity ?? 0) * 1000; // px/s
  let last = performance.now();
  let raf = 0;
  const step = (now: number) => {
    // fixed sub-steps keep it stable at 60, 120 or a hitching frame rate
    let dt = Math.min(0.05, (now - last) / 1000);
    last = now;
    while (dt > 0) {
      const h = Math.min(dt, 1 / 240);
      const a = -k * (x - opts.to) - c * v;
      v += a * h;
      x += v * h;
      dt -= h;
    }
    if (Math.abs(x - opts.to) < 0.4 && Math.abs(v) < 8) {
      opts.onUpdate(opts.to);
      opts.onDone?.();
      return;
    }
    opts.onUpdate(x);
    raf = requestAnimationFrame(step);
  };
  raf = requestAnimationFrame(step);
  return () => cancelAnimationFrame(raf);
}
