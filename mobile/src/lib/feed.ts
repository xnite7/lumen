// What Home shows: the billboard, Up Next (what you're in, the next
// episodes, new ones first, then your list), and the rows, the same mix
// the PC app builds.
import { api, invoke, type Meta, type LibraryItem, type CatalogRef } from "$lib/api";
import { orderedVideos } from "$lib/stores/playback.svelte";

export interface UpNext {
  meta: Meta;
  videoId?: string;
  progress?: number;
  label: string;
  sub?: string;
  fresh?: boolean;
}
export interface Row {
  key: string;
  title: string;
  items: Meta[];
}

const today = () => new Date().toISOString().slice(0, 10);
const daysAgo = (n: number) => new Date(Date.now() - n * 864e5).toISOString().slice(0, 10);

function metaOf(i: LibraryItem): Meta {
  return { id: i._id, type: i.type, name: i.name, poster: i.poster, background: (i as any).background, logo: (i as any).logo } as Meta;
}

export async function upNext(): Promise<UpNext[]> {
  const cw = await api.continueWatching().catch(() => [] as LibraryItem[]);
  const out: UpNext[] = [];
  const lib = await api.library().catch(() => [] as LibraryItem[]);
  const have = new Set(cw.map((i) => i._id));
  for (const i of cw) {
    const st: any = i.state ?? {};
    const d = st.duration ?? 0;
    const p = d > 0 ? Math.min(1, (st.timeOffset ?? 0) / d) : undefined;
    const vid: string | undefined = st.video_id;
    const parts = vid?.split(":") ?? [];
    const left = d > 0 ? Math.max(0, Math.round((d - (st.timeOffset ?? 0)) / 60000)) : null;
    out.push({
      meta: metaOf(i),
      videoId: parts.length === 3 ? vid : undefined,
      progress: p,
      label: parts.length === 3 ? `S${parts[1]} E${parts[2]}` : "",
      sub: left == null ? undefined : left >= 60 ? `${Math.floor(left / 60)}h ${left % 60}m left` : `${left}m left`,
    });
  }
  // shows you finished an episode of lately: their next one
  const cands = lib
    .filter((i) => i.type === "series" && !have.has(i._id) && !(i as any).removed && (i.state as any)?.video_id?.split(":").length === 3)
    .filter((i) => {
      const st: any = i.state;
      const d = st.duration ?? 0, t = st.timeOffset ?? 0;
      return t <= 0 || (d > 0 && t >= d * 0.9);
    })
    .filter((i) => Date.now() - Date.parse((i.state as any).lastWatched ?? 0) < 60 * 864e5)
    .slice(0, 10);
  const nexts = await Promise.all(
    cands.map(async (i) => {
      try {
        const full = await api.meta("series", i._id);
        const eps = orderedVideos(full).filter((v) => (v.season ?? 0) > 0);
        const cur = (i.state as any).video_id;
        const at = eps.findIndex((v) => v.id === cur);
        const seen = new Set<string>((i.state as any).watchedIds ?? []);
        const n = eps.slice(at + 1).find((v) => !seen.has(v.id) && (v.released ?? "").slice(0, 10) <= today() && (v.released ?? "") !== "");
        if (!n) return null;
        const fresh = (n.released ?? "").slice(0, 10) >= daysAgo(10);
        return { meta: { ...metaOf(i), name: full.name, logo: full.logo ?? (i as any).logo, background: full.background }, videoId: n.id, label: `S${n.season} E${n.episode ?? (n as any).number}`, sub: fresh ? "new episode" : "up next", fresh, at: Date.parse((i.state as any).lastWatched ?? 0) } as UpNext & { at: number };
      } catch {
        return null;
      }
    }),
  );
  const extra = nexts.filter(Boolean) as (UpNext & { at: number })[];
  const all = [...extra.filter((x) => x.fresh), ...out, ...extra.filter((x) => !x.fresh)];
  // a few from your list you haven't started
  try {
    const l = await api.myLists();
    const ids = new Set(all.map((x) => x.meta.id));
    for (const m of (l.plan ?? []).filter((m) => !ids.has(m.id)).slice(0, 4)) all.push({ meta: m, label: "", sub: "on your list" });
  } catch {}
  return all;
}

/** the rows under Up Next: your lists woven with couch's picks */
export async function rows(onRow: (r: Row) => void) {
  const catalogs = await api.catalogs().catch(() => [] as CatalogRef[]);
  const personal = catalogs.filter((c: any) => c.builtin && ["simkl-new", "simkl-plantowatch", "motn-top"].includes(c.id));
  const curated: any[] = await invoke<any[]>("home_rows", { scope: "all" }).catch(() => []);
  const specs: [any, [string, string][]][] = [];
  for (let i = 0; i < Math.max(personal.length, curated.length); i++) {
    if (personal[i]) specs.push([personal[i], []]);
    const r = curated[i];
    if (r) {
      const extra: [string, string][] = r.extra ?? [];
      specs.push([
        { key: `couch/${r.type}/${r.id}/${extra[0]?.[1] ?? ""}`, addonId: "com.midnite.couch.builtin", addonName: "couch", transportUrl: "couch://builtin/manifest.json", type: r.type, id: r.id, name: r.name, extra: [{ name: "skip" }], adult: false, builtin: true },
        extra,
      ]);
    }
  }
  await Promise.all(
    specs.map(async ([cat, extra], i) => {
      const items = await api.catalog(cat, extra).catch(() => [] as Meta[]);
      if (items.length) onRow({ key: `${i}:${cat.key ?? cat.id}`, title: String(cat.name ?? "").toLowerCase(), items });
    }),
  );
}
