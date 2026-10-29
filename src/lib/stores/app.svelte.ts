import { api } from "../api";
import { DEFAULT_SETTINGS, type Settings } from "../defaults";

export const app = $state({
  ready: false,
  native: false,
  platform: "",
  version: "",
  settings: structuredClone(DEFAULT_SETTINGS) as Settings,
  secrets: {} as Record<string, any>,
  /** true while a video is on screen (page goes transparent over mpv) */
  playing: false,
  fullscreen: false,
  pip: false,
  /** Adult rows revealed for this session (Hide adult toggle). */
  adultRevealed: false,
});

function merge<T>(base: T, over: any): T {
  if (!over || typeof over !== "object" || Array.isArray(over)) return (over ?? base) as T;
  const out: any = Array.isArray(base) ? [...(base as any)] : { ...(base as any) };
  for (const k of Object.keys(over)) {
    const b = (base as any)?.[k];
    out[k] = b && typeof b === "object" && !Array.isArray(b) ? merge(b, over[k]) : over[k];
  }
  return out;
}

export async function boot() {
  const [info, settings, secrets] = await Promise.all([api.appInfo(), api.settings(), api.secrets()]);
  app.native = info.native;
  app.platform = info.platform;
  app.version = info.version;
  app.settings = merge(structuredClone(DEFAULT_SETTINGS), settings);
  app.secrets = secrets ?? {};
  app.ready = true;
}

let saveTimer: ReturnType<typeof setTimeout> | undefined;

/** Settings are edited in place; this persists them, debounced. */
export function saveSettings() {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => {
    api.saveSettings($state.snapshot(app.settings)).catch((e) => console.warn("settings", e));
  }, 350);
}

export async function refreshSecrets() {
  app.secrets = (await api.secrets()) ?? {};
}
