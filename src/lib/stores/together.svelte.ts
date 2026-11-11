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

// guest steering: for a moment after, the sync doesn't pull us back
let steeredAt = 0;
/** Guest with the remote: send pause / play / seek to the host. True if sent. */
export function steer(action: "pause" | "play" | "seek", pos?: number): boolean {
  const f = together.following;
  if (!f || !together.canControl) return false;
  steeredAt = Date.now();
  if (line?.readyState === WebSocket.OPEN) line.send(JSON.stringify({ k: "cmd", a: action, p: pos }));
  else api.crewRoomCmd(f.host, action, pos).catch(() => {});
  return true;
}

// ================================================================ live line
// One WebSocket per couch, through the crew server. The host's player pushes
// its state the moment something happens (play, pause, skip, buffering) and
// once a second otherwise; guests get it pushed straight away. Round trips are
// timed so everyone aims at the same moment.
let line: WebSocket | null = null;
let lineFor = "";
let lineRole: "host" | "guest" | "" = "";
let rtt = 0.15; // seconds, smoothed
let pingTimer = 0;
let retry = 0;
let retryTimer = 0;

async function openLine(host: string, role: "host" | "guest", onMsg: (d: any) => void) {
  closeLine();
  lineFor = host;
  lineRole = role;
  let url = "";
  try {
    url = await api.crewLine(host);
  } catch {
    return scheduleRetry(host, role, onMsg);
  }
  if (lineFor !== host || lineRole !== role) return;
  const ws = new WebSocket(url);
  line = ws;
  ws.onopen = () => {
    retry = 0;
    const ping = () => ws.readyState === WebSocket.OPEN && ws.send(JSON.stringify({ k: "ping", c: performance.now() }));
    ping();
    clearInterval(pingTimer);
    pingTimer = window.setInterval(ping, 5000);
  };
  ws.onmessage = (e) => {
    let d: any;
    try {
      d = JSON.parse(e.data);
    } catch {
      return;
    }
    if (d.k === "pong") {
      const r = (performance.now() - d.c) / 1000;
      if (r > 0 && r < 5) rtt = rtt * 0.7 + r * 0.3;
      return;
    }
    onMsg(d);
  };
  ws.onclose = () => {
    if (line !== ws) return;
    line = null;
    clearInterval(pingTimer);
    if (lineFor === host && lineRole === role) scheduleRetry(host, role, onMsg);
  };
}
function scheduleRetry(host: string, role: "host" | "guest", onMsg: (d: any) => void) {
  clearTimeout(retryTimer);
  const wait = Math.min(10_000, 800 * 2 ** retry++);
  retryTimer = window.setTimeout(() => lineFor === host && lineRole === role && openLine(host, role, onMsg), wait);
}
function closeLine() {
  lineFor = "";
  lineRole = "";
  clearTimeout(retryTimer);
  clearInterval(pingTimer);
  const l = line;
  line = null;
  try {
    l?.close(1000);
  } catch {}
}
const live = () => line?.readyState === WebSocket.OPEN;

// ------------------------------------------------------------------ host
let hostTimer = 0;
let sent = { v: "", pa: true, st: false, sh: false, p: 0, at: 0 };
function hostState() {
  const s = pb.state;
  return { k: "s", v: pb.videoId, m: pb.meta?.id, ty: pb.meta?.type, ti: pb.meta?.name, p: s.pos, pa: s.paused, st: !!s.buffering && !s.paused, sh: together.shared, lag: (rtt / 2) * 1000 };
}
/** Host: put my player on the line (called when someone joins my couch). */
export function startHosting(me: string) {
  if (lineRole === "host" && lineFor === me) return;
  openLine(me, "host", (d) => {
    // a guest with the remote: the player does what they asked
    if (d.k === "cmd") window.dispatchEvent(new CustomEvent("together-cmd", { detail: { name: d.name, action: d.action, pos: d.pos } }));
  });
  clearInterval(hostTimer);
  sent = { v: "", pa: true, st: false, sh: false, p: 0, at: 0 };
  hostTimer = window.setInterval(() => {
    if (!together.hosting || !pb.open) return stopHosting();
    if (!live()) return;
    const st = hostState();
    const now = performance.now();
    // where the last report says I'd be by now
    const expect = sent.pa || sent.st ? sent.p : sent.p + (now - sent.at) / 1000;
    const changed = st.v !== sent.v || st.pa !== sent.pa || st.st !== sent.st || st.sh !== sent.sh || Math.abs(st.p - expect) > 0.75;
    if (changed || now - sent.at > 1000) {
      line!.send(JSON.stringify(st));
      sent = { v: st.v, pa: st.pa, st: st.st, sh: st.sh, p: st.p, at: now };
    }
  }, 100);
}
function stopHosting() {
  clearInterval(hostTimer);
  hostTimer = 0;
  if (lineRole === "host") closeLine();
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
      if (together.hosting) startHosting(together.hosting);
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

// ------------------------------------------------------------------ guest
// Following the host: small drift is eased out by playing a touch faster or
// slower (no jump, no rebuffer); only a big gap seeks, aimed ahead by how long
// a seek takes here, and then waits for the player to settle before it looks
// again. That waiting is what stops the "replay the same bit" loop.
let syncTimer = 0;
let pollTimer = 0;
let hostSt: null | { v: string; m: string; ty: string; ti?: string; p: number; pa: boolean; st: boolean; sh: boolean; got: number } = null;
let seekAt = 0;
let seekCost = 1.2; // seconds a seek takes to start playing again, learned
let nudged = 1;
let stalledSince = 0;
let switching = false;

function setNudge(rate: number) {
  if (Math.abs(rate - nudged) < 0.005 || pb.mode !== "native") return;
  nudged = rate;
  cmd("speed", { value: rate });
}

function takeState(d: any) {
  hostSt = { v: d.v ?? d.videoId, m: d.m ?? d.metaId, ty: d.ty ?? d.type, ti: d.ti ?? d.title, p: d.p ?? d.now, pa: !!(d.pa ?? d.paused), st: !!(d.st ?? d.stalled), sh: !!(d.sh ?? d.shared), got: performance.now() };
  together.canControl = hostSt.sh;
  // a fresh message after a pause/skip: act on it now, not on the next tick
  apply();
}

/** the host's position right now, from the last message + time since + the trip here */
function hostNow() {
  if (!hostSt) return 0;
  if (hostSt.pa || hostSt.st) return hostSt.p;
  return hostSt.p + (performance.now() - hostSt.got) / 1000 + rtt / 2;
}

async function apply() {
  const h = hostSt;
  const s = pb.state;
  if (!h || !together.following) return;
  if (!pb.open && !pb.ios) return leave();
  // they moved on to another episode: come along
  if (h.v && h.v !== pb.videoId && pb.meta && !pb.loading && !switching) {
    const v = orderedVideos(pb.meta).find((x) => x.id === h.v);
    if (v) {
      switching = true;
      setNudge(1);
      await playVideo(v).finally(() => (switching = false));
      seekAt = Date.now();
    }
    return;
  }
  if (pb.loading || switching || Date.now() - steeredAt < 1500) return;

  // a seek is still landing: let it
  const settling = Date.now() - seekAt < 10_000 && (s.buffering || Date.now() - seekAt < 700);
  if (settling) return;
  if (seekAt && Date.now() - seekAt < 12_000 && !s.buffering && !s.paused) {
    // learn how long seeks take here (aim that far ahead next time)
    const took = (Date.now() - seekAt) / 1000;
    seekCost = Math.max(0.3, Math.min(6, seekCost * 0.6 + took * 0.4));
    seekAt = 0;
  }

  const target = hostNow();
  const drift = target - s.pos; // + = I'm behind

  // host paused (or stuck loading for a moment): stop on the same frame
  const hostStopped = h.pa || (h.st && (stalledSince ||= Date.now()) && Date.now() - stalledSince > 900);
  if (!h.st) stalledSince = 0;
  if (hostStopped) {
    setNudge(1);
    if (!s.paused) cmd("pause", { value: true });
    if (Math.abs(drift) > 0.4) {
      seekAt = Date.now();
      cmd("seek", { to: target });
    }
    return;
  }
  if (h.st) return; // a blink of buffering on their side: hold course

  if (s.paused) {
    // starting together: line up first if we're off, then go
    if (Math.abs(drift) > 0.6) {
      seekAt = Date.now();
      cmd("seek", { to: target + Math.min(seekCost, 1.5) });
    }
    cmd("pause", { value: false });
    return;
  }
  if (s.buffering) return;

  const canNudge = pb.mode === "native";
  if (Math.abs(drift) > (canNudge ? 4 : 2.5)) {
    // far off (they skipped): jump, aimed where they'll be once we're playing
    setNudge(1);
    seekAt = Date.now();
    cmd("seek", { to: target + seekCost });
  } else if (canNudge && Math.abs(drift) > 0.25) {
    // close: ease in, up to ±8% speed, no jump
    setNudge(1 + Math.max(-0.08, Math.min(0.08, drift * 0.06)));
  } else if (Math.abs(drift) < 0.08) {
    setNudge(1);
  }
}

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
  hostSt = null;
  seekAt = 0;
  nudged = 1;
  await quickPlay({ id: room.metaId, type: room.type, name: room.title ?? "" } as Meta, null, { videoId: room.videoId, startAt: room.now + 2 });
  takeState(room);
  openLine(host, "guest", (d) => d.k === "s" && takeState(d));
  // the steady beat: re-check drift a few times a second; when the line is
  // down, ask the server instead (slower, still works)
  clearInterval(syncTimer);
  syncTimer = window.setInterval(() => apply(), 250);
  clearInterval(pollTimer);
  let missing = 0;
  pollTimer = window.setInterval(async () => {
    if (!together.following) return stop();
    if (live()) return void (missing = 0);
    const r = (await api.crewRoom(host).catch(() => ({ room: null }))).room;
    if (!r) {
      if (++missing > 3) {
        toast(`${name} stopped watching`, "warn");
        stop();
      }
      return;
    }
    missing = 0;
    takeState(r);
  }, 1500);
}

function stop() {
  clearInterval(syncTimer);
  clearInterval(pollTimer);
  if (lineRole === "guest") closeLine();
  hostSt = null;
  if (nudged !== 1) setNudge(1);
  together.following = null;
  together.canControl = false;
}

export async function leave() {
  const f = together.following;
  stop();
  if (f) await api.crewLeave(f.host).catch(() => {});
}
