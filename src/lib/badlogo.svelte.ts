// Title logos that wouldn't load (or whose fallback didn't either): the spot
// shows the title as text instead of a broken-image icon with alt text.
import { SvelteSet } from "svelte/reactivity";
import { imdbArt } from "./fallback";

const bad = new SvelteSet<string>();

/** true when this logo is known not to load: show the name instead */
export const logoBroken = (src?: string | null) => !src || bad.has(src);

/** use:logoOk={{ src, id }}: tries IMDb's logo once, then gives up for good */
export function logoOk(node: HTMLImageElement, o: { src?: string | null; id?: string }) {
  let opts = o;
  let tried = false;
  const fail = () => {
    const alt = imdbArt(opts.id, "logo");
    if (!tried && alt && !node.src.includes("metahub")) {
      tried = true;
      node.src = alt;
      return;
    }
    if (opts.src) bad.add(opts.src);
  };
  node.addEventListener("error", fail);
  if (node.complete && node.naturalWidth === 0 && node.getAttribute("src")) fail();
  return {
    update(n: { src?: string | null; id?: string }) {
      opts = n;
      tried = false;
    },
    destroy() {
      node.removeEventListener("error", fail);
    },
  };
}
