import { api, type LibraryItem, type Meta } from "../api";
import { toast, errorText } from "./toast.svelte";

export const lib = $state({
  items: [] as LibraryItem[],
  cw: [] as LibraryItem[],
  loaded: false,
});

export async function loadLibrary() {
  const [items, cw] = await Promise.all([api.library(), api.continueWatching()]);
  lib.items = items ?? [];
  lib.cw = cw ?? [];
  lib.loaded = true;
}

export function inLibrary(id: string) {
  return lib.items.some((i) => i._id === id && !i.removed && !i.temp);
}

export function itemOf(id: string) {
  return lib.items.find((i) => i._id === id) ?? lib.cw.find((i) => i._id === id);
}

export async function toggleLibrary(meta: Partial<Meta>) {
  const id = meta.id!;
  try {
    if (inLibrary(id)) {
      await api.libraryRemove(id, meta.type ?? "movie");
      toast(`Removed ${meta.name ?? "title"} from your library`, "info");
    } else {
      await api.libraryAdd(meta);
      toast(`Added ${meta.name ?? "title"} to your library`, "ok");
    }
    await loadLibrary();
  } catch (e) {
    toast(errorText(e), "error");
  }
}

export async function setWatched(meta: Partial<Meta>, videoId: string | null, watched: boolean) {
  try {
    await api.setWatched(meta, videoId, watched);
    await loadLibrary();
  } catch (e) {
    toast(errorText(e), "error");
  }
}

export function watchedIds(id: string): Set<string> {
  return new Set(itemOf(id)?.state?.watchedIds ?? []);
}

export function progressOf(id: string, videoId?: string): number {
  const it = itemOf(id);
  if (!it || !it.state.duration) return 0;
  if (videoId && it.state.video_id !== videoId) return 0;
  return Math.min(1, it.state.timeOffset / it.state.duration);
}
