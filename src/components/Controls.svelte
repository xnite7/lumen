<script lang="ts" module>
  // Small iOS-style form controls, exported from one file.
</script>

<script lang="ts">
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

  function set(v: any) {
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
    background: rgb(120 120 128 / 0.32);
    transition: background var(--t-med) ease;
    flex: none;
  }
  .switch.on {
    background: #34c759;
  }
  .knob {
    position: absolute;
    top: 2px;
    left: 2px;
    width: 27px;
    height: 27px;
    border-radius: 50%;
    background: #fff;
    box-shadow:
      0 3px 8px rgb(0 0 0 / 0.15),
      0 3px 1px rgb(0 0 0 / 0.06);
    transition: transform var(--t-med) var(--spring);
  }
  .on .knob {
    transform: translateX(20px);
  }
  .switch:active .knob {
    width: 32px;
  }
  .on:active .knob {
    transform: translateX(15px);
  }

  .seg {
    position: relative;
    display: inline-flex;
    padding: 2px;
    border-radius: 10px;
    background: rgb(118 118 128 / 0.24);
  }
  .seg .thumb {
    position: absolute;
    top: 2px;
    bottom: 2px;
    left: 0;
    border-radius: 8px;
    background: rgb(255 255 255 / 0.22);
    box-shadow: 0 3px 8px rgb(0 0 0 / 0.12);
    transition:
      transform var(--t-med) var(--spring),
      width var(--t-med) var(--spring);
  }
  :global([data-appearance="light"]) .seg .thumb {
    background: #fff;
  }
  .seg button {
    position: relative;
    padding: 6px 14px;
    font-size: 13px;
    font-weight: 600;
    border-radius: 8px;
    color: var(--text-dim);
    white-space: nowrap;
  }
  .seg button.active {
    color: var(--text);
  }

  .slider {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 28px;
    background: transparent;
    cursor: pointer;
  }
  .slider::-webkit-slider-runnable-track {
    height: 4px;
    border-radius: 4px;
    background: linear-gradient(90deg, var(--accent) var(--p), rgb(255 255 255 / 0.18) var(--p));
  }
  .slider::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 26px;
    height: 26px;
    margin-top: -11px;
    border-radius: 50%;
    background: #fff;
    box-shadow: 0 2px 8px rgb(0 0 0 / 0.25);
    transition: transform var(--t-fast) var(--spring);
  }
  .slider:active::-webkit-slider-thumb {
    transform: scale(1.12);
  }
</style>
