<script lang="ts" module>
  // Small iOS-style form controls, exported from one file.
</script>

<script lang="ts">
  import { haptic } from "../lib/native";
  let {
    kind,
    value = $bindable(),
    options = [],
    min = 0,
    max = 1,
    step = 0.01,
    label = "",
    onchange,
  }: {
    kind: "switch" | "segmented" | "slider";
    value: any;
    options?: { value: any; label: string }[];
    min?: number;
    max?: number;
    step?: number;
    label?: string;
    onchange?: (v: any) => void;
  } = $props();

  let segEls: HTMLButtonElement[] = $state([]);
  const idx = $derived(options.findIndex((o) => o.value === value));
  let thumb = $state({ x: 0, w: 0 });
  $effect(() => {
    const el = segEls[idx];
    if (el) thumb = { x: el.offsetLeft, w: el.offsetWidth };
  });

  let lastTick = 0;
  function set(v: any) {
    if (kind !== "slider") haptic(kind === "switch" ? "light" : "selection");
    else if (performance.now() - lastTick > 70) {
      lastTick = performance.now();
      haptic("selection");
    }
    value = v;
    onchange?.(v);
  }
</script>

{#if kind === "switch"}
  <button class="switch" class:on={value} role="switch" aria-checked={!!value} aria-label={label} onclick={() => set(!value)}>
    <span class="knob"></span>
  </button>
{:else if kind === "segmented"}
  <div class="seg" role="radiogroup" aria-label={label}>
    {#if idx >= 0}<span class="thumb" style:transform="translateX({thumb.x}px)" style:width="{thumb.w}px"></span>{/if}
    {#each options as o, i}
      <button bind:this={segEls[i]} class:active={i === idx} role="radio" aria-checked={i === idx} onclick={() => set(o.value)}>{o.label}</button>
    {/each}
  </div>
{:else}
  <input
    class="slider"
    type="range"
    {min}
    {max}
    {step}
    aria-label={label}
    value={value}
    style:--p="{((value - min) / (max - min)) * 100}%"
    oninput={(e) => set(parseFloat((e.target as HTMLInputElement).value))}
  />
{/if}

<style>
  .switch {
    position: relative;
    width: 51px;
    height: 31px;
    border-radius: 31px;
    background: var(--fill-1);
    transition: background var(--t-med) ease;
    flex: none;
  }
  .switch.on {
    background: var(--green);
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 27px;
    height: 27px;
    border-radius: 27px;
    background: #fff;
    box-shadow:
      0 3px 8px rgb(0 0 0 / 0.15),
      0 3px 1px rgb(0 0 0 / 0.06);
    transition:
      transform var(--t-med) var(--spring),
      width var(--t-med) var(--spring);
  }
  .on .knob {
    transform: translateX(20px);
  }
  .switch:active .knob {
    width: 33px;
  }
  .on:active .knob {
    transform: translateX(14px);
  }

  /* iOS 27 segmented control: capsule track, glass thumb */
  .seg {
    position: relative;
    display: inline-flex;
    padding: 2px;
    height: 32px;
    border-radius: var(--r-pill);
    background: var(--fill-3);
    max-width: 100%;
  }
  .seg .thumb {
    position: absolute;
    top: 2px;
    bottom: 2px;
    left: 0;
    border-radius: var(--r-pill);
    background: color-mix(in srgb, var(--label) 18%, transparent);
    box-shadow:
      inset 1px 1px 0 -0.5px rgb(255 255 255 / 0.3),
      0 3px 8px rgb(0 0 0 / 0.12);
    transition:
      transform var(--t-med) var(--spring),
      width var(--t-med) var(--spring);
  }
  :global([data-appearance="light"]) .seg .thumb {
    background: #fff;
  }
  .seg button {
    position: relative;
    flex: 1;
    padding: 0 14px;
    font-size: 13px;
    font-weight: 600;
    border-radius: var(--r-pill);
    color: var(--label-2);
    white-space: nowrap;
    transition: color var(--t-fast);
  }
  .seg button.active {
    color: var(--label);
  }

  .slider {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 28px;
    background: transparent;
    cursor: pointer;
    touch-action: pan-y;
  }
  .slider::-webkit-slider-runnable-track {
    height: 6px;
    border-radius: 6px;
    background: linear-gradient(90deg, var(--accent) var(--p), var(--fill-2) var(--p));
  }
  .slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 28px;
    height: 24px;
    margin-top: -9px;
    border-radius: 24px;
    background: #fff;
    box-shadow: 0 0.5px 4px rgb(0 0 0 / 0.12), 0 6px 13px rgb(0 0 0 / 0.12);
    transition: transform var(--t-fast) var(--spring);
  }
  .slider:active::-webkit-slider-thumb {
    transform: scale(1.15, 1.25);
  }
</style>
