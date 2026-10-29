// One hover preview at a time, owned by the app shell. Posters ask for it;
// the card grows out of the poster's own rectangle.

import type { Meta } from "../api";

export const preview = $state({
  meta: null as Meta | null,
  rect: null as DOMRect | null,
  source: "" as string,
  cw: null as any,
  token: 0,
});

let closeTimer: ReturnType<typeof setTimeout> | undefined;

export function openPreview(meta: Meta, el: HTMLElement, extra: { source?: string; cw?: any } = {}) {
  clearTimeout(closeTimer);
  preview.meta = meta;
  preview.rect = el.getBoundingClientRect();
  preview.source = extra.source ?? "";
  preview.cw = extra.cw ?? null;
  preview.token++;
}

export function closePreview(delay = 0) {
  clearTimeout(closeTimer);
  if (!delay) {
    preview.meta = null;
    return;
  }
  closeTimer = setTimeout(() => (preview.meta = null), delay);
}

export function keepPreview() {
  clearTimeout(closeTimer);
}
