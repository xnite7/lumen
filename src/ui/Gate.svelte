<script lang="ts">
  // couch is for the midnite Discord server. Not in it: a broken couch.
  // Signed in before the server check existed: one more sign-in.
  import { api } from "../lib/api";
  import { refreshSecrets } from "../lib/stores/app.svelte";
  import DiscordSignIn from "./DiscordSignIn.svelte";
  import mark from "../assets/brand/couch-mark.png";

  let { state, recheck }: { state: "outsider" | "signin"; recheck: () => void } = $props();

  async function other() {
    await api.discordLogout().catch(() => {});
    await refreshSecrets();
    recheck();
  }
</script>

<div class="gate" role="dialog" aria-modal="true" aria-label={state === "outsider" ? "Not a member" : "Sign in"}>
  {#if state === "outsider"}
    <div class="broken" aria-hidden="true">
      <img class="half l" src={mark} alt="" />
      <img class="half r" src={mark} alt="" />
      <span class="bit b1"></span><span class="bit b2"></span><span class="bit b3"></span>
    </div>
    <h1>sorry, you are not sigma.</h1>
    <button class="link" onclick={other}>use another Discord account</button>
  {:else}
    <img class="whole" src={mark} alt="" />
    <h1>one more knock.</h1>
    <p class="dim">couch is for the midnite server now. Sign in with Discord again so it can check you're in.</p>
    <DiscordSignIn ondone={recheck} />
  {/if}
</div>

<style>
  .gate {
    position: fixed;
    inset: 0;
    z-index: 150;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 18px;
    padding: 24px;
    text-align: center;
    background: #000;
    animation: in 500ms ease both;
  }
  @keyframes in {
    from {
      opacity: 0;
    }
  }
  h1 {
    margin: 8px 0 0;
    font-family: var(--font-bump);
    font-size: clamp(22px, 3vw, 30px);
    font-weight: 600;
    letter-spacing: -0.01em;
    text-transform: lowercase;
    color: var(--text);
  }
  p {
    margin: 0;
    max-width: 38ch;
  }
  .whole {
    width: 150px;
    filter: invert(1);
  }
  /* the couch, snapped down the middle along a jagged line */
  .broken {
    position: relative;
    width: 190px;
    height: 125px;
    margin-bottom: 26px;
  }
  .broken::before {
    content: "";
    position: absolute;
    inset: -40px;
    background: radial-gradient(closest-side, rgb(var(--neon-rgb) / 0.22), transparent);
    filter: blur(10px);
  }
  .half {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: contain;
    filter: invert(1);
    animation: snap 900ms cubic-bezier(0.2, 0.9, 0.2, 1) 250ms both;
  }
  .half.l {
    clip-path: polygon(0 0, 52% 0, 46% 22%, 55% 40%, 45% 58%, 53% 78%, 47% 100%, 0 100%);
    --to: translate(-14px, 6px) rotate(-9deg);
    transform-origin: 20% 90%;
  }
  .half.r {
    clip-path: polygon(52% 0, 100% 0, 100% 100%, 47% 100%, 53% 78%, 45% 58%, 55% 40%, 46% 22%);
    --to: translate(14px, 10px) rotate(8deg);
    transform-origin: 80% 90%;
  }
  @keyframes snap {
    0% {
      transform: none;
    }
    18% {
      transform: translateX(-2px);
    }
    26% {
      transform: translateX(2px);
    }
    100% {
      transform: var(--to);
    }
  }
  .bit {
    position: absolute;
    left: 50%;
    top: 55%;
    width: 7px;
    height: 7px;
    background: var(--text);
    clip-path: polygon(50% 0, 100% 60%, 30% 100%, 0 40%);
    opacity: 0;
    animation: fall 1200ms ease-in 450ms forwards;
  }
  .b1 {
    --x: -18px;
    --r: 140deg;
  }
  .b2 {
    --x: 6px;
    --r: -200deg;
    width: 5px;
    height: 5px;
  }
  .b3 {
    --x: 22px;
    --r: 260deg;
    width: 4px;
    height: 4px;
  }
  @keyframes fall {
    0% {
      opacity: 1;
      transform: none;
    }
    100% {
      opacity: 0.85;
      transform: translate(var(--x), 70px) rotate(var(--r));
    }
  }
  .link {
    margin-top: 6px;
    font-size: 14px;
    color: var(--text-dim);
    text-decoration: underline;
    text-underline-offset: 3px;
  }
  .link:hover {
    color: var(--text);
  }
</style>
