<script lang="ts">
  // A small symbol set drawn on a 24px grid with round 1.8 strokes, in the
  // spirit of SF Symbols. Filled variants where iOS uses them.
  let { name, size = 22, fill = false, class: cls = "" }: { name: string; size?: number; fill?: boolean; class?: string } = $props();

  const P: Record<string, string> = {
    home: "M3.5 11.2 12 4l8.5 7.2M5.8 9.6V20h4.4v-5.6h3.6V20h4.4V9.6",
    discover: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Zm3.8-12.8-2.2 5.4-5.4 2.2 2.2-5.4 5.4-2.2Z",
    library: "M4 5.5h3.2V19H4zM9.4 5.5h3.2V19H9.4zM15 6.2l3.1-.8 3 12.9-3.1.8z",
    search: "M10.6 18a7.4 7.4 0 1 0 0-14.8 7.4 7.4 0 0 0 0 14.8Zm5.3-2.1L21 21",
    settings: "M12 15.2a3.2 3.2 0 1 0 0-6.4 3.2 3.2 0 0 0 0 6.4Zm7.4-3.2c0-.5 0-.9-.1-1.3l2-1.6-2-3.4-2.4.9a7.6 7.6 0 0 0-2.2-1.3L14.3 3h-4l-.4 2.6c-.8.3-1.6.8-2.2 1.3l-2.4-.9-2 3.4 2 1.6a7.7 7.7 0 0 0 0 2.6l-2 1.6 2 3.4 2.4-.9c.7.5 1.4 1 2.2 1.3l.4 2.6h4l.4-2.6c.8-.3 1.6-.8 2.2-1.3l2.4.9 2-3.4-2-1.6c.1-.4.1-.8.1-1.3Z",
    addons: "M10 4.5a2 2 0 1 1 4 0V6h4a1 1 0 0 1 1 1v3.5h-1a2 2 0 1 0 0 4h1V18a1 1 0 0 1-1 1h-3.5v-1a2 2 0 1 0-4 0v1H6a1 1 0 0 1-1-1v-3.5h1a2 2 0 1 0 0-4H5V7a1 1 0 0 1 1-1h4V4.5Z",
    play: "M7 4.6v14.8c0 .8.9 1.3 1.6.8l11.3-7.4a1 1 0 0 0 0-1.6L8.6 3.8C7.9 3.3 7 3.8 7 4.6Z",
    pause: "M7 4.5h3.2v15H7zM13.8 4.5H17v15h-3.2z",
    plus: "M12 5v14M5 12h14",
    check: "M5 12.5l4.4 4.4L19 7.3",
    close: "M6 6l12 12M18 6 6 18",
    back: "M15 5l-7 7 7 7",
    forward: "M9 5l7 7-7 7",
    chevronDown: "M6 9l6 6 6-6",
    chevronUp: "M6 15l6-6 6 6",
    more: "M6 12h.01M12 12h.01M18 12h.01",
    heart: "M12 20s-7.5-4.4-7.5-10A4.3 4.3 0 0 1 12 7.3 4.3 4.3 0 0 1 19.5 10c0 5.6-7.5 10-7.5 10Z",
    star: "M12 3.6l2.6 5.3 5.8.8-4.2 4.1 1 5.8L12 16.9l-5.2 2.7 1-5.8-4.2-4.1 5.8-.8L12 3.6Z",
    eye: "M2.5 12S6 5.5 12 5.5 21.5 12 21.5 12 18 18.5 12 18.5 2.5 12 2.5 12Zm9.5 3a3 3 0 1 0 0-6 3 3 0 0 0 0 6Z",
    eyeOff: "M3 3l18 18M10.6 5.6A9.9 9.9 0 0 1 12 5.5c6 0 9.5 6.5 9.5 6.5a17 17 0 0 1-3 3.7M6.6 6.6A16.6 16.6 0 0 0 2.5 12S6 18.5 12 18.5c1.6 0 3-.4 4.3-1.1M9.9 9.9a3 3 0 0 0 4.2 4.2",
    volume: "M4 9.5h3.5L12 5.5v13l-4.5-4H4zM15.5 9a4 4 0 0 1 0 6M18 6.5a7.5 7.5 0 0 1 0 11",
    mute: "M4 9.5h3.5L12 5.5v13l-4.5-4H4zM16 9.5l5 5M21 9.5l-5 5",
    subtitles: "M3.5 6h17v12h-17zM7 14h4M13.5 14H17M7 10.5h2.5M12 10.5h5",
    audio: "M9 18.5V6.5l10-2v12M9 18.5a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0Zm10-2a2.5 2.5 0 1 1-5 0 2.5 2.5 0 0 1 5 0Z",
    fullscreen: "M4 9V4h5M20 9V4h-5M4 15v5h5M20 15v5h-5",
    exitFullscreen: "M9 4v5H4M15 4v5h5M9 20v-5H4M15 20v-5h5",
    pip: "M3.5 5.5h17v13h-17zM12.5 12h6v4.5h-6z",
    skipF: "M12.5 5.5a6.5 6.5 0 1 1-6.4 7.7M12.5 5.5V2.8L16 5.5l-3.5 2.6V5.5Z",
    skipB: "M11.5 5.5a6.5 6.5 0 1 0 6.4 7.7M11.5 5.5V2.8L8 5.5l3.5 2.6V5.5Z",
    next: "M5.5 5v14l9.5-7zM17.5 5v14",
    trash: "M5 7h14M9.5 7V4.5h5V7M7 7l1 13h8l1-13",
    refresh: "M20 11a8 8 0 0 0-14.6-4.5M4 4.5v4h4M4 13a8 8 0 0 0 14.6 4.5M20 19.5v-4h-4",
    link: "M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1 1M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1-1",
    sparkles: "M12 3.5l1.6 4.4 4.4 1.6-4.4 1.6L12 15.5l-1.6-4.4L6 9.5l4.4-1.6L12 3.5ZM18.5 15l.8 2.2 2.2.8-2.2.8-.8 2.2-.8-2.2-2.2-.8 2.2-.8.8-2.2Z",
    bulb: "M9 18h6M10 21h4M12 3a6 6 0 0 0-3.6 10.8c.7.5 1.1 1.3 1.1 2.2h5c0-.9.4-1.7 1.1-2.2A6 6 0 0 0 12 3Z",
    grip: "M9 6h.01M15 6h.01M9 12h.01M15 12h.01M9 18h.01M15 18h.01",
    drag: "M5 9h14M5 15h14",
    bolt: "M13 3 5 13.5h6L10 21l8-10.5h-6L13 3Z",
    info: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Zm0-11v6m0-9h.01",
    globe: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18ZM3.5 9h17M3.5 15h17M12 3c2.4 2.4 3.6 5.4 3.6 9s-1.2 6.6-3.6 9c-2.4-2.4-3.6-5.4-3.6-9S9.6 5.4 12 3Z",
    minimize: "M6 12h12",
    maximize: "M6.5 6.5h11v11h-11z",
    film: "M4 4.5h16v15H4zM8 4.5v15M16 4.5v15M4 9h4M4 15h4M16 9h4M16 15h4",
    tv: "M3.5 6h17v11h-17zM8.5 20.5h7M12 17v3.5",
    person: "M12 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8Zm-7.5 8.5c.8-3.6 3.9-6 7.5-6s6.7 2.4 7.5 6",
    download: "M12 4v11M7.5 10.5 12 15l4.5-4.5M5 19.5h14",
    upload: "M12 15V4M7.5 8.5 12 4l4.5 4.5M5 19.5h14",
    gauge: "M4.5 17a8 8 0 1 1 15 0M12 13l4-4.5",
    magnet: "M6 4.5h4v7a2 2 0 0 0 4 0v-7h4v7a6 6 0 0 1-12 0v-7ZM6 8h4M14 8h4",
    palette: "M12 21a9 9 0 1 1 9-9c0 2-1.6 3-3.2 3H16a1.6 1.6 0 0 0-1.2 2.7c.6.7.4 3.3-2.8 3.3ZM7.5 11h.01M10 7.5h.01M14.5 7.5h.01M17 11h.01",
  };
  const filled = new Set(["play", "pause", "next", "bolt"]);
  const d = $derived(P[name] ?? P.info);
  const isFill = $derived(fill || filled.has(name));
</script>

<svg
  class={cls}
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill={isFill ? "currentColor" : "none"}
  stroke="currentColor"
  stroke-width={isFill ? 0 : 1.8}
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"><path {d} /></svg>
