// Watch together. Ask to join a friend's couch; they let you in (or not);
// then your player follows theirs: same title, same episode, same moment,
// paused when they pause.

import { api, type CrewAsk, type CrewMember, type Meta } from "../api";
import { crew } from "./crew.svelte";
import { pb, cmd, playVideo, orderedVideos } from "./playback.svelte";
import { quickPlay } from "../quickplay";
import { toast } from "./toast.svelte";
import { haptic } from "../native";
import { loadMe } from "./social.svelte";

export const together = $state({
  /** people asking to join my couch */
  inbox: [] as CrewAsk[],
  /** who I've asked, waiting on an answer */
  asking: null as null | { id: string; host: CrewMember },
  /** whose couch I'm on */
  following: null as null | { host: string; name: string },
  /** my own id while friends are on my couch (the room is mine) */
  hosting: null as null | string,
  /** host: everyone can pause / play / seek */
  shared: false,
  /** guest: the host shared the remote */
  canControl: false,
});

/** Host: hand everyone the remote (or take it back). */
export function setShared(on: boolean) {
  together.shared = on;
  api.crewShareControl(on).catch(() => {});
  toast(on ? "everyone on your couch can control playback" : "you've got the remote back", "ok");
}

// guest steering: for a few seconds after, the sync doesn't pull us back
let steeredAt = 0;
/** Guest with the remote: send pause / play / seek to the host. True if sent. */
export function steer(action: "pause" | "play" | "seek", pos?: number): boolean {
  const f = together.following;
  if (!f || !together.canControl) return false;
  steeredAt = Date.now();
  api.crewRoomCmd(f.host, action, pos).catch(() => {});
  return true;
}

let inboxTimer = 0;
export function startTogether() {
  if (inboxTimer) return;
  inboxTimer = window.setInterval(async () => {
    if (!crew.setup || document.visibilityState !== "visible") return;
    try {
      const r = await api.crewInbox();
      const fresh = (r.asks ?? []).filter((a) => !together.inbox.some((x) => x.id === a.id));
      if (fresh.length) haptic("success");
      together.inbox = r.asks ?? [];
    } catch {
      /* the couch is quiet */
    }
  }, 5000);
}

export async function answer(a: CrewAsk, accept: boolean) {
  together.inbox = together.inbox.filter((x) => x.id !== a.id);
  try {
    await api.crewAnswer(a.id, accept);
    if (accept) {
      toast(`${a.fromName} is on your couch`, "ok");
      together.hosting = (await loadMe())?.id ?? null;
    }
  } catch (e) {
    toast(String(e), "error");
  }
}

export async function askToJoin(host: CrewMember) {
  if (together.asking) return;
  haptic("medium");
  let id: string;
  try {
    id = (await api.crewAsk(host.id)).id;
  } catch (e) {
    toast(String(e), "error");
    return;
  }
  together.asking = { id, host };
  const until = Date.now() + 120_000;
  while (together.asking?.id === id && Date.now() < until) {
    await new Promise((r) => setTimeout(r, 2000));
    let status = "pending";
    try {
      status = (await api.crewAskStatus(id)).status;
    } catch {
      continue;
    }
    if (status === "accepted") {
      together.asking = null;
      return follow(host.id, host.name);
    }
    if (status !== "pending") {
      together.asking = null;
      toast(
        status === "declined" ? `${host.name} wants the couch to themselves` : status === "ended" ? `${host.name} stopped watching` : "that ask expired",
        "warn",
      );
      return;
    }
  }
  if (together.asking?.id === id) {
    together.asking = null;
    toast(`${host.name} didn't answer`, "warn");
  }
}

export function cancelAsk() {
  const a = together.asking;
  together.asking = null;
  if (a) api.crewCancelAsk(a.id).catch(() => {});
}

let syncTimer = 0;
async function follow(host: string, name: string) {
  // the host's player reports within a second or two
  let room = null;
  for (let i = 0; i < 10 && !room; i++) {
    room = (await api.crewRoom(host).catch(() => ({ room: null }))).room;
    if (!room) await new Promise((r) => setTimeout(r, 1000));
  }
  if (!room) {
    toast(`couldn't catch up with ${name}`, "error");
    return;
  }
  together.following = { host, name };
  toast(`watching with ${name}`, "ok");
  await quickPlay({ id: room.metaId, type: room.type, name: room.title ?? "" } as Meta, null, { videoId: room.videoId, startAt: room.now + 2 });
  clearInterval(syncTimer);
  let missing = 0;
  syncTimer = window.setInterval(async () => {
    if (!together.following) return stop();
    if (!pb.open && !pb.ios) return leave();
    const r = (await api.crewRoom(host).catch(() => ({ room: null }))).room;
    if (!r) {
      if (++missing > 3) {
        toast(`${name} stopped watching`, "warn");
        stop();
      }
      return;
    }
    missing = 0;
    together.canControl = !!r.shared;
    // they moved on to another episode: come along
    if (r.videoId !== pb.videoId && pb.meta && !pb.loading) {
      const v = orderedVideos(pb.meta).find((x) => x.id === r.videoId);
      if (v) await playVideo(v);
      return;
    }
    if (pb.loading || Date.now() - steeredAt < 3500) return;
    if (r.paused !== pb.state.paused) cmd("pause", { value: r.paused });
    const drift = r.now + 0.3 - pb.state.pos;
    if (Math.abs(drift) > 2.5) cmd("seek", { to: r.now + 0.3 });
  }, 2000);
}

function stop() {
  clearInterval(syncTimer);
  together.following = null;
  together.canControl = false;
}

export async function leave() {
  const f = together.following;
  stop();
  if (f) await api.crewLeave(f.host).catch(() => {});
}
