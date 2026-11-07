// Play straight from a billboard: no detail page, no stream list. couch
// fetches the streams, ranks them for your connection and starts the best,
// with the rest lined up as fallbacks. Shows pick up where you left off.

import { api, listen, uid, type Meta, type Stream } from "./api";
import { app } from "./stores/app.svelte";
import { play, orderedVideos, pb } from "./stores/playback.svelte";
import { itemOf } from "./stores/library.svelte";
import { connectionMbps, rank, needMbps } from "./autopick";
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

export async function quickPlay(base: Meta, from?: HTMLElement | null, opts: { videoId?: string; startAt?: number } = {}): Promise<void> {
  const s = app.settings.streams;
  // straight into the player's loading screen while couch finds the stream
  pb.meta = base;
  pb.error = "";
  pb.loading = true;
  pb.open = true;
  let meta: Meta = base;
  try {
    meta = await api.meta(base.type, base.id, base._addon);
  } catch {
    /* the catalog entry will do */
  }
  let videoId = meta.id;
  let episodeTitle: string | undefined;
  let runtime: any = meta.runtime;
  if (opts.videoId && opts.videoId !== meta.id) {
    // a specific episode (watching along with someone)
    const v = orderedVideos(meta).find((x) => x.id === opts.videoId);
    videoId = opts.videoId;
    episodeTitle = v ? v.title || v.name || `Episode ${v.episode ?? v.number}` : undefined;
    runtime = (v as any)?.runtime ?? runtime;
  } else if (meta.type === "series" && !opts.videoId) {
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
    pb.open = false;
    pb.loading = false;
    toast("couldn't find a stream for that one", "error");
    return openDetail(base, from);
  }
  await play(meta, order[0], videoId, { episodeTitle, queue: order.slice(1), startAt: opts.startAt });
  watchForBetter(meta, videoId, order, runtime, episodeTitle);
}

// ------------------------------------------------- better picture later
// Started on a slow connection? Every few minutes couch checks again, and if
// the Wi-Fi can now keep up with a better stream it switches over at the
// same moment.
let upgradeTimer = 0;
function watchForBetter(meta: Meta, videoId: string, order: Stream[], runtime: any, episodeTitle?: string) {
  clearInterval(upgradeTimer);
  const best = (order[0]?._couch?.resRank ?? 0);
  const top = Math.max(...order.map((x) => x._couch?.resRank ?? 0));
  if (best >= top) return; // already the best there is
  upgradeTimer = window.setInterval(async () => {
    if (!pb.open || pb.videoId !== videoId) return clearInterval(upgradeTimer);
    if (pb.loading || pb.state.paused || pb.state.pos < 60) return;
    const cur = pb.stream?._couch?.resRank ?? 0;
    const mbps = await connectionMbps(true);
    const better = order.find((x) => (x._couch?.resRank ?? 0) > cur && x._couch?.kind !== "torrent" && needMbps(x, runtime) <= mbps * 0.7);
    if (!better || !pb.open || pb.videoId !== videoId) return;
    const at = pb.state.pos;
    toast(`your connection got better — switching to ${better._couch?.res ?? "a better stream"}`, "ok");
    await play(meta, better, videoId, { episodeTitle, queue: order.filter((x) => x !== better), startAt: at });
    if ((better._couch?.resRank ?? 0) >= top) clearInterval(upgradeTimer);
  }, 6 * 60 * 1000);
}
