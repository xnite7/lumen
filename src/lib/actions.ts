// Quick actions shared by every poster, card and hero (long-press menu).
import { img, type Meta } from "./api";
import type { MenuItem } from "./menu.svelte";
import { openDetail } from "./open";
import { inLibrary, toggleLibrary, setWatched, itemOf } from "./stores/library.svelte";
import { share } from "./native";
import { toast } from "./stores/toast.svelte";

export function posterMenu(m: Meta, extra: MenuItem[] = []) {
  const owned = inLibrary(m.id);
  const watched = !!itemOf(m.id)?.state?.flaggedWatched;
  const items: MenuItem[] = [
    { label: "Open", icon: "info", run: () => openDetail(m) },
    { label: owned ? "Remove from My List" : "Add to My List", icon: owned ? "check" : "plus", run: () => toggleLibrary(m) },
    {
      label: watched ? "Mark as unwatched" : "Mark as watched",
      icon: "checkCircle",
      run: async () => {
        await setWatched(m, null, !watched);
        toast(watched ? "Marked unwatched" : "Marked watched", "ok", undefined, 1500);
      },
    },
    ...extra,
    {
      label: "Share",
      icon: "share",
      run: () => share({ title: m.name, url: m.id.startsWith("tt") ? `https://www.imdb.com/title/${m.id.split(":")[0]}/` : undefined }),
    },
  ];
  return { items, title: m.name, image: img(m.poster ?? m.background, "w500"), shape: (m.posterShape ?? "poster") as "poster" | "landscape" | "square" };
}
