// A small stack router. Every navigation runs inside a View Transition so the
// browser morphs between the old and new page (and a clicked poster tagged
// `hero-art` grows into the detail page's hero).

import { tick } from "svelte";
import { app } from "./stores/app.svelte";

export type Route =
  | { name: "home" }
  | { name: "discover"; type?: string; catalog?: string }
  | { name: "library" }
  | { name: "search"; q?: string }
  | { name: "detail"; type: string; id: string; hint?: string; preview?: any }
  | { name: "person"; person: string; tmdbId?: number; adult?: boolean }
  | { name: "addons"; install?: string }
  | { name: "settings"; section?: string }
  | { name: "vibe"; q?: string };

export const tabs = ["home", "discover", "library", "search"] as const;

export const nav = $state({
  stack: [{ name: "home" }] as Route[],
  scroll: new Map<number, number>(),
});

export function current(): Route {
  return nav.stack[nav.stack.length - 1];
}

function withTransition(update: () => void, back = false) {
  const doc = document as any;
  const reduce = app.settings.general.motionScale === 0 || matchMedia("(prefers-reduced-motion: reduce)").matches;
  // WebKitGTK (Linux) crashes inside startViewTransition on some drivers;
  // WebView2 (Windows) and WKWebView (iOS) are fine.
  if (!doc.startViewTransition || reduce || app.platform === "linux") {
    update();
    return;
  }
  document.documentElement.classList.toggle("vt-back", back);
  // Rendering is paused inside the callback, so wait on Svelte's flush --
  // not on a frame, which would never come.
  const t = doc.startViewTransition(async () => {
    update();
    await tick();
  });
  t.finished.finally(() => document.documentElement.classList.remove("vt-back"));
}

export function go(route: Route, opts: { replace?: boolean } = {}) {
  const scroller = document.querySelector("[data-scroller]") as HTMLElement | null;
  if (scroller) nav.scroll.set(nav.stack.length - 1, scroller.scrollTop);
  const isTab = (tabs as readonly string[]).includes(route.name) || route.name === "settings" || route.name === "addons";
  if (isTab) nav.scroll.clear();
  withTransition(() => {
    if (isTab) nav.stack = [route];
    else if (opts.replace) nav.stack = [...nav.stack.slice(0, -1), route];
    else nav.stack = [...nav.stack, route];
  });
}

export function back() {
  if (nav.stack.length <= 1) return false;
  withTransition(() => {
    nav.stack = nav.stack.slice(0, -1);
  }, true);
  return true;
}

export function savedScroll(): number {
  return nav.scroll.get(nav.stack.length - 1) ?? 0;
}
