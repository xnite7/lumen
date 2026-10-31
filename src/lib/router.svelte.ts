// A small stack router per tab. Pushes slide in from the right, pops slide
// back, tab switches cross-fade -- each inside a View Transition. Each tab
// keeps its own stack and scroll position, like UITabBarController.

import { tick } from "svelte";
import { app } from "./stores/app.svelte";
import { haptic } from "./native";

export type Route =
  | { name: "home" }
  | { name: "search"; q?: string }
  | { name: "library" }
  | { name: "profile" }
  | { name: "discover"; type?: string; catalog?: string; genre?: string }
  | { name: "detail"; type: string; id: string; hint?: string; preview?: any }
  | { name: "person"; person: string; tmdbId?: number; adult?: boolean }
  | { name: "addons"; install?: string }
  | { name: "settings"; section?: string }
  | { name: "vibe"; q?: string };

/** a route on a stack carries a stable key, so its page layer survives */
export type Entry = Route & { _k?: number };
let nextKey = 1;
const keyed = (r: Route): Entry => ({ ...r, _k: nextKey++ });

export const tabs = ["home", "search", "library", "profile"] as const;
export type Tab = (typeof tabs)[number];

const stacks: Record<Tab, Entry[]> = {
  home: [keyed({ name: "home" })],
  search: [keyed({ name: "search" })],
  library: [keyed({ name: "library" })],
  profile: [keyed({ name: "profile" })],
};
/** scroll position per page layer, kept across tab switches */
export const scrolls = new Map<number, number>();

export const nav = $state({
  tab: "home" as Tab,
  stack: stacks.home as Entry[],
});

export function current(): Entry {
  return nav.stack[nav.stack.length - 1];
}

function scroller() {
  return document.querySelector("[data-scroller]") as HTMLElement | null;
}
function remember() {
  const s = scroller();
  const k = current()._k;
  if (s && k) scrolls.set(k, s.scrollTop);
}

function withTransition(update: () => void, kind: "push" | "pop" | "tab") {
  const doc = document as any;
  const reduce = app.settings.general.motionScale === 0 || matchMedia("(prefers-reduced-motion: reduce)").matches;
  // WebKitGTK (Linux) crashes inside startViewTransition on some drivers.
  if (!doc.startViewTransition || reduce || app.platform === "linux") {
    update();
    return;
  }
  const html = document.documentElement;
  html.classList.toggle("vt-back", kind === "pop");
  html.classList.toggle("vt-tab", kind === "tab");
  const t = doc.startViewTransition(async () => {
    update();
    await tick();
  });
  t.finished.finally(() => html.classList.remove("vt-back", "vt-tab"));
}

const isTab = (n: string): n is Tab => (tabs as readonly string[]).includes(n);

/** Switch tab. Tapping the current tab pops it to its root (iOS). */
export function selectTab(tab: Tab) {
  if (tab === nav.tab) {
    if (nav.stack.length > 1) {
      withTransition(() => {
        nav.stack = [nav.stack[0]];
        stacks[tab] = nav.stack;
      }, "pop");
    } else {
      scroller()?.scrollTo({ top: 0, behavior: "smooth" });
    }
    return;
  }
  remember();
  haptic("selection");
  stacks[nav.tab] = nav.stack;
  withTransition(() => {
    nav.tab = tab;
    nav.stack = stacks[tab];
  }, "tab");
}

export function go(route: Route, opts: { replace?: boolean } = {}) {
  if (isTab(route.name)) {
    selectTab(route.name);
    // carry parameters (e.g. a search query) into the tab root
    if (Object.keys(route).length > 1) nav.stack = [keyed(route), ...nav.stack.slice(1)];
    return;
  }
  remember();
  withTransition(() => {
    nav.stack = opts.replace ? [...nav.stack.slice(0, -1), keyed(route)] : [...nav.stack, keyed(route)];
    stacks[nav.tab] = nav.stack;
  }, "push");
}

/** `animate: false` when a gesture already animated the pop itself. */
export function back(opts: { animate?: boolean } = {}) {
  if (nav.stack.length <= 1) return false;
  const pop = () => {
    nav.stack = nav.stack.slice(0, -1);
    stacks[nav.tab] = nav.stack;
  };
  if (opts.animate === false) pop();
  else withTransition(pop, "pop");
  return true;
}
