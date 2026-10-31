// Native iPhone features (Swift plugin "lumen-native"). Every call is a
// no-op elsewhere, so callers never branch on platform.

import { invoke, isTauri } from "./api";
import { addPluginListener } from "@tauri-apps/api/core";
import { app } from "./stores/app.svelte";

const ios = () => isTauri && app.platform === "ios";
/** UI decisions (the browser mock can pretend with ?platform=ios). */
const iosUI = () => app.platform === "ios";

async function call<T = void>(cmd: string, args: Record<string, unknown> = {}): Promise<T | undefined> {
  if (!ios()) return undefined;
  try {
    return await invoke<T>(`plugin:lumen-native|${cmd}`, args);
  } catch (e) {
    console.warn("native", cmd, e);
    return undefined;
  }
}

export type Haptic = "light" | "medium" | "heavy" | "soft" | "rigid" | "selection" | "success" | "warning" | "error";

/** Taptic feedback. On desktop a selection tick is meaningless, so nothing. */
export function haptic(style: Haptic = "light") {
  if (ios()) void call("haptic", { style });
  else if (typeof navigator !== "undefined" && "vibrate" in navigator && matchMedia("(pointer: coarse)").matches) {
    try {
      navigator.vibrate(style === "selection" ? 4 : 8);
    } catch {}
  }
}

export async function share(opts: { title?: string; text?: string; url?: string }) {
  if (ios()) return call("share", opts);
  if (navigator.share) return navigator.share(opts).catch(() => {});
  if (opts.url) await navigator.clipboard?.writeText(opts.url);
}

/** Safari sheet inside the app (sign-in pages). */
export async function openAuth(url: string): Promise<boolean> {
  if (!ios()) return false;
  await call("auth", { url });
  return true;
}
export const closeAuth = () => call("endauth");

export interface NativePlay {
  url: string;
  title: string;
  subtitle?: string;
  artwork?: string;
  startAt?: number;
  startPct?: number;
  tint?: string;
  headers?: Record<string, string>;
  engine?: string;
  subLangs?: string[];
  audioLangs?: string[];
}
/** AVPlayer, full screen: PiP, AirPlay, Now Playing, Live Activity. */
export const nativePlay = (req: NativePlay) => call("play", req as any);
export const nativeStop = () => call("stop");

export function onNative<T = any>(event: string, cb: (p: T) => void) {
  if (!ios()) return Promise.resolve(() => {});
  return addPluginListener<T>("lumen-native", event, cb).then((l) => () => l.unregister());
}

export const isIOS = iosUI;
