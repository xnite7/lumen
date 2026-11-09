// The people and studios you follow. Their new releases get a row on Home,
// and they show on your couch page.
import { api, type Follow } from "../api";
import { toast, errorText } from "./toast.svelte";

export const follows = $state({ list: [] as Follow[], loaded: false });

export async function loadFollows(force = false) {
  if (follows.loaded && !force) return;
  follows.list = (await api.follows().catch(() => [])) ?? [];
  follows.loaded = true;
}

export const isFollowing = (kind: Follow["kind"], id?: number | null) => !!id && follows.list.some((f) => f.kind === kind && f.id === id);

export async function toggleFollow(f: Follow) {
  const was = isFollowing(f.kind, f.id);
  // instant on screen
  follows.list = was ? follows.list.filter((x) => !(x.kind === f.kind && x.id === f.id)) : [f, ...follows.list];
  try {
    follows.list = (await api.followToggle(f)) ?? follows.list;
    toast(was ? `unfollowed ${f.name}` : `following ${f.name}. new stuff shows up on Home`, "ok");
  } catch (e) {
    toast(errorText(e), "error");
    await loadFollows(true);
  }
}
