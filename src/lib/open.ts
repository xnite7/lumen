import type { Meta } from "./api";
import { go } from "./router.svelte";
import { closePreview, preview } from "./stores/preview.svelte";

/**
 * Navigate to a title. The clicked artwork is tagged `hero-art` for the
 * duration of the transition so it visibly grows into the detail hero.
 */
export function openDetail(meta: Meta, from?: HTMLElement | null) {
  const fromPreview = !!preview.meta && !!from?.closest(".pv");
  // a preview stays on screen for the "before" picture, so it visibly grows
  // into the page; it goes in the same step the page arrives
  if (!fromPreview) closePreview(0, false);
  const art = (from?.querySelector("img.pv-art") ?? from?.querySelector("img")) as HTMLElement | null;
  if (art) {
    art.style.viewTransitionName = "hero-art";
    setTimeout(() => (art.style.viewTransitionName = ""), 900);
  }
  go({ name: "detail", type: meta.type, id: meta.id, hint: meta._addon, preview: meta }, { during: () => closePreview(0, false) });
}
