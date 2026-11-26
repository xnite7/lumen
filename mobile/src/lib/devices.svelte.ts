// Your own devices, Spotify-style: this iPhone / iPad says what it's playing
// every few seconds; another device playing shows as "playing on …" with
// play/pause and "play here" (the film moves over at the same spot).
import { api, type Meta } from "$lib/api";
import { pb, playNext } from "$lib/stores/playback.svelte";
import { nativeControl, nativeStop } from "$lib/native";
import { quickPlay } from "$lib/quickplay";
import { toast } from "$lib/stores/toast.svelte";

export interface Device {
  id: string;
  name: string;
  kind: string;
  playing: any;
}

export const devices = $state({ others: [] as Device[], pending: null as null | { id: string; paused: boolean; at: number } });

const KEY = "couch-device-id";
function myId(): string {
  let id = "";
  try {
    id = localStorage.getItem(KEY) ?? "";
  } catch {}
  if (!id) {
    id = Array.from(crypto.getRandomValues(new Uint8Array(8)), (b) => b.toString(16).padStart(2, "0")).join("");
    try {
      localStorage.setItem(KEY, id);
    } catch {}
  }
  return id;
}

const ipad = () => /iPad/.test(navigator.userAgent) || (/Macintosh/.test(navigator.userAgent) && navigator.maxTouchPoints > 1);

/** the stream, small (what the other device needs to find the same one) */
function tag(s: any) {
  if (!s) return null;
  const t = String(s.title ?? s.name ?? "").slice(0, 80);
  if (s.infoHash) return { h: String(s.infoHash).toLowerCase(), f: s.fileIdx ?? null, a: s._addonName ?? null, t };
  if (s.url && String(s.url).length < 480) return { u: s.url, a: s._addonName ?? null, t };
  return { a: s._addonName ?? null, t };
}

function playing() {
  if (!pb.meta || !(pb.ios || pb.open)) return null;
  const m = pb.meta;
  const parts = (pb.videoId ?? "").split(":");
  const episode = parts.length === 3 ? `S${parts[1]} E${parts[2]}${pb.episodeTitle ? ` · ${pb.episodeTitle}` : ""}` : "";
  return {
    metaId: String(m.id).split(":")[0], type: m.type, title: m.name, episode, videoId: pb.videoId,
    poster: m.poster, background: m.background, logo: m.logo,
    pos: pb.state.pos, duration: pb.state.duration, paused: pb.state.paused, stream: tag(pb.stream),
  };
}

function apply(c: any) {
  if (!pb.ios && !pb.open) return;
  switch (c?.cmd) {
    case "pause":
      nativeControl("pause", { flag: true });
      break;
    case "play":
      nativeControl("pause", { flag: false });
      break;
    case "seek":
      if (typeof c.pos === "number") nativeControl("seek", { value: c.pos });
      break;
    case "next":
      playNext();
      break;
    case "stop":
      nativeStop();
      toast("carrying on on your other device", "info");
      break;
  }
}

let started = false;
export function startDevices() {
  if (started) return;
  started = true;
  const id = myId();
  const tick = async () => {
    try {
      const r = await api.social<any>("PUT", "/v1/me/device", { id, name: ipad() ? "iPad" : "iPhone", kind: ipad() ? "ipad" : "iphone", playing: playing() });
      devices.others = (r?.devices ?? []).filter((d: any) => !d.me);
      for (const c of r?.cmds ?? []) apply(c);
    } catch {}
    const busy = devices.others.some((d) => d.playing);
    setTimeout(tick, document.visibilityState === "visible" ? (busy ? 2000 : 4000) : 8000);
  };
  setTimeout(tick, 1500);
}

export function send(dev: string, cmd: "pause" | "play" | "seek" | "stop" | "next", pos?: number) {
  return api.social("POST", `/v1/me/device/${dev}/cmd`, { cmd, pos }).catch(() => {});
}

export async function playHere(d: Device) {
  const p = d.playing;
  const meta = { id: p.metaId, type: p.type ?? "movie", name: p.title ?? "", poster: p.poster, background: p.background, logo: p.logo } as Meta;
  const vid = typeof p.videoId === "string" && p.videoId.split(":").length === 3 ? p.videoId : undefined;
  await send(d.id, "stop");
  devices.others = devices.others.filter((x) => x.id !== d.id);
  await quickPlay(meta, null, { videoId: vid, startAt: p.pos > 1 ? p.pos : undefined });
}
