// tvOS-style focus for Windows: a focused card lifts, tilts toward the
// pointer, catches a sheen, and glows neon underneath. Arrow keys and a
// controller move focus between cards and rows like a TV app.

import { app } from "./stores/app.svelte";
import { click } from "./sound";

type Opts = { tilt?: boolean; max?: number };
const items = new Set<HTMLElement>();
let current: HTMLElement | null = null;

function setFocus(el: HTMLElement | null, scroll = false) {
  if (current && current !== el) current.classList.remove("focus");
  current = el;
  if (!el) return;
  el.classList.add("focus");
  if (scroll) {
    el.focus({ preventScroll: true });
    el.scrollIntoView({ block: "nearest", inline: "nearest", behavior: "smooth" });
    if (app.settings.general.sound) click();
  }
}

/** use:tvFocus on any card or button that should feel like a TV tile. */
export function tvFocus(node: HTMLElement, opts: Opts = {}) {
  const fine = matchMedia("(hover: hover) and (pointer: fine)").matches;
  let o = { tilt: true, max: 7, ...opts };
  node.classList.add("tv");
  node.dataset.tv = "";
  if (!node.querySelector(":scope > .tv-sheen")) {
    const sheen = document.createElement("span");
    sheen.className = "tv-sheen";
    sheen.setAttribute("aria-hidden", "true");
    node.appendChild(sheen);
  }
  items.add(node);

  const move = (e: PointerEvent) => {
    if (!fine) return;
    const r = node.getBoundingClientRect();
    const px = (e.clientX - r.left) / r.width;
    const py = (e.clientY - r.top) / r.height;
    node.style.setProperty("--mx", `${(px * 100).toFixed(1)}%`);
    node.style.setProperty("--my", `${(py * 100).toFixed(1)}%`);
    if (o.tilt) {
      node.style.setProperty("--ry", `${((px - 0.5) * o.max * 2).toFixed(2)}deg`);
      node.style.setProperty("--rx", `${(-(py - 0.5) * o.max * 2).toFixed(2)}deg`);
    }
  };
  const enter = () => fine && setFocus(node);
  const leave = () => {
    node.style.removeProperty("--rx");
    node.style.removeProperty("--ry");
    if (current === node) setFocus(null);
  };
  const focus = () => setFocus(node);
  const blur = () => current === node && setFocus(null);

  node.addEventListener("pointermove", move);
  node.addEventListener("pointerenter", enter);
  node.addEventListener("pointerleave", leave);
  node.addEventListener("focus", focus);
  node.addEventListener("blur", blur);
  return {
    update(n: Opts) {
      o = { tilt: true, max: 7, ...n };
    },
    destroy() {
      items.delete(node);
      if (current === node) current = null;
      node.removeEventListener("pointermove", move);
      node.removeEventListener("pointerenter", enter);
      node.removeEventListener("pointerleave", leave);
      node.removeEventListener("focus", focus);
      node.removeEventListener("blur", blur);
    },
  };
}

// ------------------------------------------------------- spatial navigation

type Dir = "left" | "right" | "up" | "down";

function visible(el: HTMLElement) {
  if (!el.isConnected) return false;
  const r = el.getBoundingClientRect();
  if (r.width === 0 || r.height === 0) return false;
  // only the page on top of the stack (not the one under a swipe, not hidden layers)
  const slot = el.closest(".slot");
  if (slot && !slot.classList.contains("top")) return false;
  return r.bottom > -r.height * 2 && r.top < innerHeight + r.height * 2;
}

function nearest(from: DOMRect, dir: Dir): HTMLElement | null {
  const cx = from.left + from.width / 2;
  const cy = from.top + from.height / 2;
  let best: HTMLElement | null = null;
  let score = Infinity;
  for (const el of items) {
    if (el === current || !visible(el)) continue;
    const r = el.getBoundingClientRect();
    const x = r.left + r.width / 2;
    const y = r.top + r.height / 2;
    const dx = x - cx;
    const dy = y - cy;
    let main: number, cross: number;
    if (dir === "right") [main, cross] = [dx, dy];
    else if (dir === "left") [main, cross] = [-dx, dy];
    else if (dir === "down") [main, cross] = [dy, dx];
    else [main, cross] = [-dy, dx];
    if (main <= 4) continue;
    // rows matter: moving sideways stays on the row, moving up/down prefers
    // the card most directly above or below
    const s = main + Math.abs(cross) * (dir === "left" || dir === "right" ? 4 : 0.6);
    if (s < score) {
      score = s;
      best = el;
    }
  }
  return best;
}

export function moveFocus(dir: Dir) {
  if (!current || !current.isConnected || !visible(current)) {
    const first = [...items].filter(visible).sort((a, b) => a.getBoundingClientRect().top - b.getBoundingClientRect().top)[0];
    if (first) setFocus(first, true);
    return;
  }
  const next = nearest(current.getBoundingClientRect(), dir);
  if (next) setFocus(next, true);
}

export function activateFocus() {
  current?.click();
}

const KEYS: Record<string, Dir> = { ArrowLeft: "left", ArrowRight: "right", ArrowUp: "up", ArrowDown: "down" };

/** Installed once from App on desktop. Returns a cleanup. */
export function installTvNav(opts: { blocked: () => boolean; back: () => void }) {
  const key = (e: KeyboardEvent) => {
    if (opts.blocked() || e.altKey || e.ctrlKey || e.metaKey) return;
    const t = e.target as HTMLElement;
    if (t?.closest("input, textarea, select, [contenteditable], [role=slider]")) return;
    const d = KEYS[e.key];
    if (d) {
      e.preventDefault();
      moveFocus(d);
    } else if ((e.key === "Enter" || e.key === " ") && current && document.activeElement !== current) {
      e.preventDefault();
      activateFocus();
    }
  };
  addEventListener("keydown", key);

  // controller: D-pad / left stick to move, A to select, B to go back
  let raf = 0;
  let last = 0;
  const held: Record<string, boolean> = {};
  const poll = (now: number) => {
    raf = requestAnimationFrame(poll);
    const pads = navigator.getGamepads?.() ?? [];
    const g = [...pads].find(Boolean);
    if (!g || opts.blocked()) return;
    const ax = g.axes[0] ?? 0;
    const ay = g.axes[1] ?? 0;
    const b = (i: number) => !!g.buttons[i]?.pressed;
    const dir: Dir | null = b(14) || ax < -0.6 ? "left" : b(15) || ax > 0.6 ? "right" : b(12) || ay < -0.6 ? "up" : b(13) || ay > 0.6 ? "down" : null;
    if (dir && now - last > (held.dir ? 140 : 260)) {
      moveFocus(dir);
      last = now;
      held.dir = true;
    } else if (!dir) held.dir = false;
    if (b(0) && !held.a) activateFocus();
    if (b(1) && !held.b) opts.back();
    held.a = b(0);
    held.b = b(1);
  };
  const connected = () => {
    if (!raf) raf = requestAnimationFrame(poll);
  };
  addEventListener("gamepadconnected", connected);
  if ([...(navigator.getGamepads?.() ?? [])].some(Boolean)) connected();
  return () => {
    removeEventListener("keydown", key);
    removeEventListener("gamepadconnected", connected);
    cancelAnimationFrame(raf);
  };
}
