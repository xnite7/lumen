// One context menu for the whole app: long-press (touch) or right-click.
// The pressed element lifts out of the page as the preview, iOS style.

import { haptic } from "./native";

export interface MenuItem {
  label: string;
  icon?: string;
  destructive?: boolean;
  disabled?: boolean;
  run: () => void;
}

export const menu = $state({
  open: false,
  items: [] as MenuItem[],
  title: "",
  rect: null as DOMRect | null,
  image: "" as string | undefined,
  shape: "poster" as "poster" | "landscape" | "square",
});

export function openMenu(el: HTMLElement | null, items: MenuItem[], opts: { title?: string; image?: string; shape?: "poster" | "landscape" | "square" } = {}) {
  menu.rect = el?.getBoundingClientRect() ?? null;
  menu.items = items;
  menu.title = opts.title ?? "";
  menu.image = opts.image;
  menu.shape = opts.shape ?? "poster";
  menu.open = true;
  haptic("medium");
}

export function closeMenu() {
  menu.open = false;
}

/**
 * `use:pressMenu={() => ({ items, title, image })}` -- long-press on touch,
 * right-click with a mouse. A long-press suppresses the click that follows.
 */
export function pressMenu(node: HTMLElement, get: () => { items: MenuItem[]; title?: string; image?: string; shape?: "poster" | "landscape" | "square" } | null) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let fired = false;
  let sx = 0;
  let sy = 0;
  const fire = () => {
    const m = get();
    if (!m) return;
    fired = true;
    openMenu(node, m.items, m);
  };
  const down = (e: PointerEvent) => {
    if (e.pointerType === "mouse") return;
    fired = false;
    sx = e.clientX;
    sy = e.clientY;
    node.classList.add("pressing");
    timer = setTimeout(fire, 480);
  };
  const move = (e: PointerEvent) => {
    if (timer && Math.hypot(e.clientX - sx, e.clientY - sy) > 10) cancel();
  };
  const cancel = () => {
    clearTimeout(timer);
    timer = undefined;
    node.classList.remove("pressing");
  };
  const click = (e: MouseEvent) => {
    if (fired) {
      e.preventDefault();
      e.stopImmediatePropagation();
      fired = false;
    }
  };
  const ctx = (e: MouseEvent) => {
    e.preventDefault();
    cancel();
    fire();
    fired = false;
  };
  node.addEventListener("pointerdown", down);
  node.addEventListener("pointermove", move);
  node.addEventListener("pointerup", cancel);
  node.addEventListener("pointercancel", cancel);
  node.addEventListener("pointerleave", cancel);
  node.addEventListener("click", click, true);
  node.addEventListener("contextmenu", ctx);
  return {
    destroy() {
      cancel();
      node.removeEventListener("pointerdown", down);
      node.removeEventListener("pointermove", move);
      node.removeEventListener("pointerup", cancel);
      node.removeEventListener("pointercancel", cancel);
      node.removeEventListener("pointerleave", cancel);
      node.removeEventListener("click", click, true);
      node.removeEventListener("contextmenu", ctx);
    },
  };
}
