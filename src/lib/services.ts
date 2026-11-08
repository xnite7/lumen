// A streaming service's shelf as a catalog couch's built-in addon serves
// (TMDB: what's on that service in the US, most popular first).
import type { CatalogRef } from "./api";

export function serviceCatalog(id: number, type: "movie" | "series", name: string): CatalogRef {
  return {
    key: `couch/${type}/couch-prov-${id}`,
    addonId: "com.midnite.couch.builtin",
    addonName: "couch",
    transportUrl: "couch://builtin/manifest.json",
    type,
    id: `couch-prov-${id}`,
    name,
    extra: [{ name: "skip" } as any],
    adult: false,
    builtin: true,
  };
}
