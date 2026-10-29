// Themes are data: a set of CSS custom properties plus optional extra CSS.
// Drop a .json (and optionally a same-named .css) into the themes folder
// and it shows up in Settings → Appearance. No build step, no reload.

export interface Theme {
  id: string;
  name: string;
  author?: string;
  appearance: "dark" | "light";
  /** CSS variable name (without --) -> value */
  tokens: Record<string, string>;
  effects?: { grain?: number; ambient?: number };
  css?: string;
  user?: boolean;
  preview?: string[];
}

export const BUILTIN_THEMES: Theme[] = [
  {
    id: "liquid-glass",
    name: "Liquid Glass",
    author: "Lumen",
    appearance: "dark",
    tokens: {},
    effects: { grain: 0.07, ambient: 0.75 },
    preview: ["#0e1220", "#3a4157", "#ffc22e"],
  },
  {
    // From your "cozy innocent death" palette. The dread comes from too much
    // light: blown highlights, warm haze, TV grain -- and no pure black.
    id: "cozy-innocent-death",
    name: "Cozy Innocent Death",
    author: "xnite",
    appearance: "light",
    tokens: {
      ink: "#f3e3b4",
      "ink-2": "#ead49a",
      text: "#2b2416",
      "text-dim": "rgb(43 36 22 / 0.66)",
      "text-faint": "rgb(43 36 22 / 0.42)",
      line: "rgb(43 36 22 / 0.12)",
      "glass-tint": "rgb(255 246 214 / 0.46)",
      "glass-tint-strong": "rgb(255 243 205 / 0.8)",
      "glass-rim": "rgb(255 255 255 / 0.55)",
      "glass-spec": "rgb(255 255 255 / 0.9)",
      "glass-shadow": "0 18px 46px -20px rgb(120 84 20 / 0.45), 0 2px 8px -2px rgb(120 84 20 / 0.2)",
      "accent-ink": "#2b2416",
      "font-display": "'Iowan Old Style', 'Palatino Linotype', Palatino, Georgia, serif",
    },
    effects: { grain: 0.16, ambient: 0.55 },
    css: `
      body { background: radial-gradient(120% 90% at 70% -10%, #FFE9A8 0%, #FFC22E 38%, #f0c46a 55%, #77B9E4 120%); }
      .grain { mix-blend-mode: multiply; }
      .ambient-layer { filter: saturate(.7) sepia(.25) brightness(1.15); }
      .hero-shade { background: linear-gradient(0deg, #f3e3b4 0%, rgb(243 227 180 / 0.6) 30%, transparent 70%) !important; }
      .btn.primary { background: #2b2416; color: #FFE9A8; }
      .scanlines { opacity: .12 !important; }
    `,
    preview: ["#FFE9A8", "#FFC22E", "#7DBF3F", "#77B9E4", "#E4736A"],
  },
  {
    id: "midnight",
    name: "Midnight",
    author: "Lumen",
    appearance: "dark",
    tokens: {
      ink: "#06080e",
      "ink-2": "#0c0f18",
      "glass-tint": "rgb(14 16 24 / 0.55)",
      "glass-tint-strong": "rgb(10 12 18 / 0.85)",
      "glass-rim": "rgb(255 255 255 / 0.09)",
      "glass-spec": "rgb(255 255 255 / 0.18)",
    },
    effects: { grain: 0.03, ambient: 0.35 },
    preview: ["#06080e", "#1a1f2e", "#9fb4ff"],
  },
  {
    id: "broadcast",
    name: "Broadcast",
    author: "Lumen",
    appearance: "dark",
    tokens: {
      ink: "#0b140d",
      "ink-2": "#122016",
      text: "#e9f7dc",
      "text-dim": "rgb(233 247 220 / 0.62)",
      "glass-tint": "rgb(18 34 22 / 0.5)",
      "glass-rim": "rgb(125 191 63 / 0.25)",
      "glass-spec": "rgb(200 255 170 / 0.3)",
      "font-display": "'Cascadia Code', 'SF Mono', Consolas, monospace",
    },
    effects: { grain: 0.14, ambient: 0.45 },
    css: `
      .scanlines { opacity: .22 !important; }
      .large-title, .title1 { text-shadow: 0 0 18px rgb(125 191 63 / .45); }
    `,
    preview: ["#0b140d", "#7DBF3F", "#e9f7dc"],
  },
  {
    id: "daylight",
    name: "Daylight",
    author: "Lumen",
    appearance: "light",
    tokens: {
      ink: "#eef1f6",
      "ink-2": "#e2e7ef",
      text: "#121722",
      "text-dim": "rgb(18 23 34 / 0.62)",
      "text-faint": "rgb(18 23 34 / 0.4)",
      line: "rgb(18 23 34 / 0.1)",
      "glass-tint": "rgb(255 255 255 / 0.55)",
      "glass-tint-strong": "rgb(255 255 255 / 0.85)",
      "glass-rim": "rgb(255 255 255 / 0.75)",
      "glass-spec": "rgb(255 255 255 / 1)",
      "glass-shadow": "0 16px 40px -18px rgb(30 40 70 / 0.35), 0 2px 6px -2px rgb(30 40 70 / 0.15)",
    },
    effects: { grain: 0.03, ambient: 0.4 },
    css: `.hero-shade { background: linear-gradient(0deg, #eef1f6 0%, rgb(238 241 246 / 0.55) 32%, transparent 72%) !important; }`,
    preview: ["#eef1f6", "#ffffff", "#2f6bff"],
  },
];

let applied: string[] = [];

export function applyTheme(t: Theme, opts: { grain: boolean; ambient: boolean }) {
  const root = document.documentElement;
  for (const k of applied) root.style.removeProperty(`--${k}`);
  applied = Object.keys(t.tokens);
  for (const [k, v] of Object.entries(t.tokens)) root.style.setProperty(`--${k}`, v);
  root.dataset.theme = t.id;
  root.dataset.appearance = t.appearance;
  root.style.colorScheme = t.appearance;
  root.style.setProperty("--grain-opacity", String(opts.grain ? (t.effects?.grain ?? 0.07) : 0));
  root.style.setProperty("--ambient-strength", String(opts.ambient ? (t.effects?.ambient ?? 0.7) : 0));
  let style = document.getElementById("theme-css") as HTMLStyleElement | null;
  if (!style) {
    style = document.createElement("style");
    style.id = "theme-css";
    document.head.appendChild(style);
  }
  style.textContent = t.css ?? "";
}

/** A starting point for someone writing their own theme file. */
export function themeTemplate(base: Theme): Theme {
  return {
    id: "my-theme",
    name: "My theme",
    author: "",
    appearance: base.appearance,
    tokens: {
      ink: "#0e1220",
      text: "#f4f6fb",
      "text-dim": "rgb(244 246 251 / 0.62)",
      "glass-tint": "rgb(30 34 50 / 0.42)",
      "glass-rim": "rgb(255 255 255 / 0.16)",
      "glass-blur": "26px",
      "r-md": "18px",
      "poster-r": "14px",
      ...base.tokens,
    },
    effects: { grain: 0.07, ambient: 0.7 },
    css: "/* any extra CSS */",
  };
}
