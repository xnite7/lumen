// Getting around: a tab bar (iPhone) or a sidebar (iPad), and on top of a
// tab a stack of pages that slide in and swipe back out.
import { api, type Meta } from "$lib/api";

export type Tab = "home" | "search" | "couch" | "lists" | "me";
export type Page =
  | { name: "detail"; meta: Meta }
  | { name: "member"; id: string; title?: string }
  | { name: "list"; title: string; items: Meta[] };

export const nav = $state({
  tab: "home" as Tab,
  stack: [] as Page[],
  /** the screen is wide enough for the iPad layout */
  wide: false,
});

export function go(p: Page) {
  nav.stack = [...nav.stack, p];
}
export function back() {
  nav.stack = nav.stack.slice(0, -1);
}
export function tab(t: Tab) {
  if (nav.tab === t && nav.stack.length) nav.stack = [];
  nav.tab = t;
}
export async function openTitle(meta: Meta) {
  // TMDB picks ("more like this") need their real id first
  if (String(meta.id).startsWith("tmdb:")) {
    try {
      const r = await api.resolve(meta.type, meta.id);
      meta = { ...meta, id: r.id, type: r.type };
    } catch {
      return;
    }
  }
  go({ name: "detail", meta });
}
