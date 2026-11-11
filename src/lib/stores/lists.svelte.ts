// couch's own custom lists (for everyone, no Simkl PRO needed). The + on a
// title offers them; without any, + just adds to My List (Plan to Watch).
import { api, type CouchList, type Meta } from "../api";
import { inLibrary, toggleLibrary } from "./library.svelte";
import { openMenu } from "../menu.svelte";
import { toast, errorText } from "./toast.svelte";

export const lists = $state({ all: [] as CouchList[], loaded: false });
/** the "name your list" sheet: open with what to put in it first */
export const namer = $state({ open: false, first: null as Partial<Meta> | null, rename: null as null | { id: string; name: string } });
/** new list (optionally with a first title), or rename one */
export function askName(first: Partial<Meta> | null = null, rename: { id: string; name: string } | null = null) {
  namer.first = first;
  namer.rename = rename;
  namer.open = true;
}

export async function loadLists() {
  if (lists.loaded) return;
  lists.all = (await api.couchLists().catch(() => [])) ?? [];
  lists.loaded = true;
}
async function save() {
  try {
    await api.saveCouchLists($state.snapshot(lists.all) as CouchList[]);
  } catch (e) {
    toast(errorText(e), "error");
  }
}
const slim = (m: Partial<Meta>) => ({ id: m.id, type: m.type, name: m.name, poster: m.poster, background: m.background, logo: m.logo, releaseInfo: m.releaseInfo }) as Meta;

export function newList(name: string, first?: Partial<Meta>) {
  const n = name.trim().slice(0, 60);
  if (!n) return null;
  const l: CouchList = { id: Math.random().toString(36).slice(2, 10), name: n, items: first ? [slim(first)] : [], at: Date.now() };
  lists.all = [l, ...lists.all];
  save();
  return l;
}
export function renameList(id: string, name: string) {
  const l = lists.all.find((x) => x.id === id);
  if (!l || !name.trim()) return;
  l.name = name.trim().slice(0, 60);
  save();
}
export function deleteList(id: string) {
  lists.all = lists.all.filter((x) => x.id !== id);
  save();
}
export const inList = (l: CouchList, id?: string) => !!id && l.items.some((i) => i.id === id);
export function toggleInList(l: CouchList, m: Partial<Meta>) {
  if (inList(l, m.id)) {
    l.items = l.items.filter((i) => i.id !== m.id);
    toast(`removed from ${l.name}`, "info");
  } else {
    l.items = [slim(m), ...l.items];
    toast(`added to ${l.name}`, "ok");
  }
  l.at = Date.now();
  save();
}

/** the + button: lists to pick from if you have any, else straight to My List */
export async function addTo(m: Partial<Meta>, el?: HTMLElement | null) {
  await loadLists();
  if (!lists.all.length) return toggleLibrary(m);
  const owned = inLibrary(m.id!);
  openMenu(
    el ?? null,
    [
      { label: owned ? "My List · remove" : "My List", icon: owned ? "check" : "plus", run: () => toggleLibrary(m) },
      ...lists.all.map((l) => ({ label: inList(l, m.id) ? `${l.name} · remove` : l.name, icon: inList(l, m.id) ? "check" : "plus", run: () => toggleInList(l, m) })),
      {
        label: "New list…",
        icon: "plus",
        run: () => askName(m),
      },
    ],
    { title: m.name ?? "Add to", image: m.poster },
  );
}
