// Tobii events from Rust, plus the head-tracked "window" parallax.

import { listen } from "../api";
import { app } from "./app.svelte";

export const gaze = $state({
  x: -1,
  y: -1,
  inside: false,
  present: true,
  head: { x: 0, y: 0, z: 600 },
  active: false,
  status: "off",
});

let started = false;

export function initGaze() {
  if (started) return;
  started = true;
  listen<{ x: number; y: number; inside: boolean }>("tobii://gaze", (p) => {
    gaze.x = p.x;
    gaze.y = p.y;
    gaze.inside = p.inside;
    gaze.active = true;
  });
  listen<{ present: boolean }>("tobii://presence", (p) => (gaze.present = p.present));
  listen<{ x: number; y: number; z: number }>("tobii://head", (p) => {
    gaze.head = p;
    headSeen = performance.now();
    wake();
  });
  listen<{ status: string }>("tobii://status", (p) => (gaze.status = p.status));
  if (import.meta.env.DEV) (window as any).__gaze = gaze;
}

// ------------------------------------------------------------ head window
//
// The screen as a window: move your head right and the artwork behind it
// slides left, the text in front drifts right, lean in and it opens up a
// little. Head position comes in millimetres from the tracker; "centre" is
// wherever you actually sit (a slow running average), so it works without
// calibrating and follows you when you shift in your chair.

type Layer = { node: HTMLElement; depth: number };
const layers = new Set<Layer>();
let headSeen = 0;
let raf = 0;
let centre: { x: number; y: number; z: number } | null = null;
const cur = { x: 0, y: 0, z: 0 };

function on() {
  return app.settings.tobii.enabled && app.settings.tobii.headParallax;
}

function wake() {
  if (!raf && layers.size && on()) raf = requestAnimationFrame(frame);
}

function frame() {
  raf = 0;
  const h = gaze.head;
  const fresh = performance.now() - headSeen < 600;
  if (fresh) {
    if (!centre) centre = { ...h };
    // ~8 s to re-centre: slow enough that leaning reads as movement
    centre.x += (h.x - centre.x) * 0.002;
    centre.y += (h.y - centre.y) * 0.002;
    centre.z += (h.z - centre.z) * 0.002;
  }
  const k = app.settings.tobii.parallaxStrength;
  const clamp = (v: number) => Math.max(-1, Math.min(1, v));
  const tx = fresh && centre ? clamp((h.x - centre.x) / 160) : 0;
  const ty = fresh && centre ? clamp((h.y - centre.y) / 110) : 0;
  const tz = fresh && centre ? clamp((centre.z - h.z) / 180) : 0;
  // critically-damped-ish follow: smooth, no lag you notice
  cur.x += (tx - cur.x) * 0.18;
  cur.y += (ty - cur.y) * 0.18;
  cur.z += (tz - cur.z) * 0.12;
  for (const l of layers) {
    const d = l.depth;
    const x = -cur.x * 34 * d * k;
    const y = cur.y * 22 * d * k;
    const s = d > 0 ? 1 + 0.07 * d + cur.z * 0.04 * d * k : 1;
    l.node.style.transform = `translate3d(${x.toFixed(2)}px, ${y.toFixed(2)}px, 0) scale(${s.toFixed(4)})`;
  }
  const settling = Math.abs(tx - cur.x) + Math.abs(ty - cur.y) + Math.abs(tz - cur.z) > 0.002;
  if (on() && (fresh || settling)) raf = requestAnimationFrame(frame);
  else if (!on()) for (const l of layers) l.node.style.transform = "";
}

/**
 * use:parallax={depth}: 1 = far background (moves against your head, with
 * overscan so edges never show), negative = foreground (moves with you).
 */
export function parallax(node: HTMLElement, depth = 1) {
  const l: Layer = { node, depth };
  layers.add(l);
  node.style.willChange = "transform";
  wake();
  return {
    update(d: number) {
      l.depth = d;
    },
    destroy() {
      layers.delete(l);
    },
  };
}
