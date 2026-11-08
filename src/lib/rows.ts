// Which rows go where: Home gets your own lists plus couch's curated rows;
// the Shows and Movies tabs get every catalog of their kind.

import { api, invoke, type CatalogRef } from "./api";
import { app } from "./stores/app.svelte";

const BUILTIN = "couch://builtin/manifest.json";

export interface RowSpec {
  catalog: CatalogRef;
  title?: string;
  extra?: [string, string][];
  link?: boolean;
}

/** couch's hand-picked rows, scoped to "all" (Home), "series" or "movie". */
export async function curated(scope: "all" | "series" | "movie"): Promise<RowSpec[]> {
  let rows: { id: string; type: string; name: string; extra?: [string, string][] }[] = [];
  try {
    rows = await invoke<any[]>("home_rows", { scope });
  } catch {
    return [];
  }
  return rows.map((r) => ({
    catalog: {
      key: `couch/${r.type}/${r.id}/${r.extra?.[0]?.[1] ?? ""}`,
      addonId: "com.midnite.couch.builtin",
      addonName: "couch",
      transportUrl: BUILTIN,
      type: r.type,
      id: r.id,
      name: r.name,
      extra: [{ name: "skip" } as any],
      adult: false,
      builtin: true,
    },
    extra: r.extra,
    link: false,
  }));
}

/** your own rows (Simkl lists, Top 10) belong on Home */
const personal = (c: CatalogRef) => c.builtin && /^(simkl-(upnext|new|watching|plantowatch)|motn-top)$/.test(c.id);
/** Simkl's Up Next + Watching live inside couch's own Continue Watching row */
const folded = (c: CatalogRef) => c.builtin && /^simkl-(upnext|watching)$/.test(c.id);

export async function catalogRows(where: "home" | "series" | "movie"): Promise<CatalogRef[]> {
  const all = await api.catalogs();
  const browsable = all.filter((c) => !c.extra.some((e) => e.isRequired && (e.name === "search" || !e.options?.length)));
  const h = app.settings.home;
  const hidden = new Set(h.hiddenRows);
  const order = new Map(h.rowOrder.map((k, i) => [k, i]));
  return browsable
    .filter((c) => !hidden.has(c.key))
    .filter((c) => !folded(c))
    .filter((c) => (where === "home" ? personal(c) : c.type === where && !personal(c)))
    .map((c, i) => ({ c, i }))
    .sort((a, b) => (order.get(a.c.key) ?? 1000 + a.i) - (order.get(b.c.key) ?? 1000 + b.i))
    .map((x) => x.c);
}
