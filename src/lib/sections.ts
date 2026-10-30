// Settings sections, as they appear in Profile.
import settingsSource from "../views/Settings.svelte?raw";

export interface Section {
  id: string;
  label: string;
  icon: string;
  tint: string;
  /** platforms where it applies; omitted = everywhere */
  only?: ("windows" | "browser" | "ios" | "linux")[];
}

export const SECTIONS: Section[] = [
  { id: "appearance", label: "Appearance", icon: "palette", tint: "var(--indigo)" },
  { id: "playback", label: "Playback", icon: "play", tint: "var(--pink)" },
  { id: "subtitles", label: "Subtitles", icon: "subtitles", tint: "var(--blue)" },
  { id: "streams", label: "Streams", icon: "film", tint: "var(--orange)" },
  { id: "home", label: "Home rows", icon: "rows", tint: "var(--teal)" },
  { id: "accounts", label: "Accounts", icon: "personCircle", tint: "#30b0c7" },
  { id: "integrations", label: "Services & keys", icon: "sparkles", tint: "var(--purple)" },
  { id: "hue", label: "Philips Hue", icon: "bulb", tint: "#e5b800" },
  { id: "tobii", label: "Eye tracking", icon: "eye", tint: "var(--green)", only: ["windows", "browser", "linux"] },
  { id: "network", label: "Network", icon: "globe", tint: "var(--blue)" },
  { id: "storage", label: "Storage", icon: "storage", tint: "var(--gray)" },
  { id: "about", label: "About", icon: "info", tint: "var(--gray)" },
];

export const sectionById = (id: string) => SECTIONS.find((s) => s.id === id);

/**
 * Every setting's label, found by reading the Settings page source at build
 * time -- so search can never fall out of date with the page.
 */
export const SETTINGS_INDEX: { label: string; hint: string; section: string }[] = (() => {
  const out: { label: string; hint: string; section: string }[] = [];
  const parts = settingsSource.split(/\{(?:#|:else )if section === "([a-z]+)"\}/);
  for (let i = 1; i < parts.length; i += 2) {
    const section = parts[i];
    const body = parts[i + 1] ?? "";
    for (const m of body.matchAll(/<Cell label="([^"{]+)"(?: hint="([^"{]*)")?/g)) out.push({ label: m[1], hint: m[2] ?? "", section });
    for (const m of body.matchAll(/class="group-head">([^<{]+)</g)) out.push({ label: m[1].trim(), hint: "", section });
  }
  return out;
})();
