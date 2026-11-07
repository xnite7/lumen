// A genre tapped somewhere else (a title page) opens that genre's page on
// the Shows or Movies tab.
import { selectTab } from "./router.svelte";

export const wantGenre = $state<{ series: string; movie: string }>({ series: "", movie: "" });

export function openGenre(kind: string, g: string) {
  const k = kind === "series" ? "series" : "movie";
  wantGenre[k] = g;
  selectTab(k === "series" ? "shows" : "movies");
}
