// One hover preview at a time, owned by the app shell. Posters ask for it;
// the card grows out of the poster's own rectangle and shrinks back into it.

import type { Meta } from "../api";

export const preview = $state({
  meta: null as Meta | null,
  rect: null as DOMRect | null,
  /** the artwork the poster was showing, so the card starts as the poster */
  art: "" as string,
  source: "" as string,
  cw: null as any,
  token: 0,
  closing: false,
});

let closeTimer: ReturnType<typeof setTimeout> | undefined;
/** the poster the card grew out of: hidden while the card covers it */
let fromEl: HTMLElement | null = null;
function showFrom() {
  if (fromEl) fromEl.style.visibility = "";
  fromEl = null;
}

// any scroll (the page or a row) puts the card away at once
if (typeof document !== "undefined") {
  document.addEventListener(
    "scroll",
    (e) => {
      if (!preview.meta || (e.target instanceof Element && e.target.closest(".pv"))) return;
      closePreview(0, false);
    },
    { capture: true, passive: true },
  );
}
let goneTimer: ReturnType<typeof setTimeout> | undefined;
let lastClosed = 0;

/** Every poster waits its own full dwell: no chaining from one preview into
 *  the next, each hover starts fresh. */
export function hoverDelay(base: number): number {
  return base;
}

export function openPreview(meta: Meta, el: HTMLElement, extra: { source?: string; cw?: any } = {}) {
  clearTimeout(closeTimer);
  clearTimeout(goneTimer);
  preview.closing = false;
  preview.meta = meta;
  preview.rect = el.getBoundingClientRect();
  showFrom();
  fromEl = el;
  // one frame later, so the card's first frame (at the poster's size) covers it
  requestAnimationFrame(() => fromEl === el && preview.meta && (el.style.visibility = "hidden"));
  preview.art = (el.querySelector("img") as HTMLImageElement | null)?.currentSrc ?? "";
  preview.source = extra.source ?? "";
  preview.cw = extra.cw ?? null;
  preview.token++;
}

/** `delay` before it starts shrinking back; `animate: false` drops it at once. */
export function closePreview(delay = 0, animate = true) {
  clearTimeout(closeTimer);
  const shut = () => {
    if (!preview.meta) return;
    if (!animate) {
      clearTimeout(goneTimer);
      showFrom();
      preview.meta = null;
      preview.closing = false;
      lastClosed = performance.now();
      return;
    }
    preview.closing = true;
    clearTimeout(goneTimer);
    goneTimer = setTimeout(() => {
      showFrom();
      preview.meta = null;
      preview.closing = false;
      lastClosed = performance.now();
    }, 240);
  };
  if (!delay) shut();
  else closeTimer = setTimeout(shut, delay);
}

export function keepPreview() {
  clearTimeout(closeTimer);
  if (preview.closing) {
    clearTimeout(goneTimer);
    preview.closing = false;
  }
}
