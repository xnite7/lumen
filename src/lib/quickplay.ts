// Play straight from a billboard: no detail page, no stream list. couch
// fetches the streams, ranks them for your connection and starts the best,
// with the rest lined up as fallbacks. Shows pick up where you left off.

import { api, listen, uid, type Meta, type Stream } from "./api";
import { app } from "./stores/app.svelte";
import { play, orderedVideos } from "./stores/playback.svelte";
import { itemOf } from "./stores/library.svelte";
import { connectionMbps, rank } from "./autopick";
import { toast } from "./stores/toast.svelte";
import { openDetail } from "./open";

function gather(type: string, id: string, ms = 9000): Promise<Stream[]> {
  return new Promise((resolve) => {
    const reqId = uid();
    let got: Stream[] = [];
    let un: (() => void) | undefined;
    const done = () => {
      clearTimeout(t);
      un?.();
      resolve(got);
    };
    const t = setTimeout(done, ms);
    listen<any>("streams", (c) => {
      if (c.reqId !== reqId) return;
      if (c.finished) return done();
      got = [...got, ...(c.streams ?? [])];
    }).then((u) => {
      un = u;
      api.streams(type, id, reqId);
    });
  });
}

export async function quickPlay(base: Meta, from?: HTMLElement | null): Promise<void> {
  const s = app.settings.streams;
  let meta: Meta = base;
  try {
    meta = await api.meta(base.type, base.id, base._addon);
  } catch {
    /* the catalog entry will do */
  }
  let videoId = meta.id;
  let episodeTitle: string | undefined;
  let runtime: any = meta.runtime;
  if (meta.type === "series") {
    const list = orderedVideos(meta).filter((v) => (v.season ?? 0) > 0);
    const at = itemOf(meta.id)?.state?.video_id;
    const v = list.find((x) => x.id === at) ?? list[0];
    if (!v) return openDetail(base, from);
    videoId = v.id;
    episodeTitle = v.title || v.name || `Episode ${v.episode ?? v.number}`;
    runtime = (v as any).runtime ?? runtime;
  }
  const [streams, mbps] = await Promise.all([gather(meta.type, videoId), connectionMbps()]);
  const ok = streams.filter((x) => {
    const p = x._couch;
    if (!p) return true;
    if (s.hideCam && p.cam) return false;
    if (s.hide3d && p.threeD) return false;
    if (s.maxSizeGb > 0 && p.sizeBytes && p.sizeBytes > s.maxSizeGb * 1e9) return false;
    return true;
  });
  const order = rank(ok, mbps, runtime, app.platform === "ios");
  if (!order.length) {
    toast("couldn't find a stream for that one", "error");
    return openDetail(base, from);
  }
  await play(meta, order[0], videoId, { episodeTitle, queue: order.slice(1) });
}
