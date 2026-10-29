import type { Meta } from "./api";
import { go } from "./router.svelte";
import { closePreview } from "./stores/preview.svelte";

/**
 * Navigate to a title. The clicked artwork is tagged `hero-art` for the
 * duration of the transition so it visibly grows into the detail hero.
 */
export function openDetail(meta: Meta, from?: HTMLElement | null) {
  closePreview();
  const art = from?.querySelector("img") as HTMLElement | null;
  if (art) {
    art.style.viewTransitionName = "hero-art";
    setTimeout(() => (art.style.viewTransitionName = ""), 900);
  }
  go({ name: "detail", type: meta.type, id: meta.id, hint: meta._addon, preview: meta });
}
