import { api } from "../api";
import { app } from "./app.svelte";
import { BUILTIN_THEMES, applyTheme, type Theme } from "../themes";
import { installSprings } from "../motion";

export const themes = $state({ list: [...BUILTIN_THEMES] as Theme[] });

export async function loadThemes() {
  try {
    const user = (await api.themesUser()) as Theme[];
    themes.list = [...BUILTIN_THEMES, ...user.filter((t) => t && t.id && t.tokens)];
  } catch {
    themes.list = [...BUILTIN_THEMES];
  }
}

export function applyCurrent() {
  const g = app.settings.general;
  const t = themes.list.find((x) => x.id === g.theme) ?? BUILTIN_THEMES[0];
  applyTheme(t, { grain: g.grain, ambient: g.ambientGlow });
  document.documentElement.classList.toggle("refraction", g.refraction);
  installSprings(g.motionScale);
}
