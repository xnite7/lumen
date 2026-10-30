// Shell state shared by the floating bars and pages.
export const ui = $state({
  scrollY: 0,
  /** scrolling down past the fold: the tab bar shrinks (iOS 26 behaviour) */
  minimized: false,
  /** onboarding on screen */
  onboarding: false,
  /** width class: "phone" < 760px <= "wide" */
  phone: typeof window !== "undefined" ? window.innerWidth < 760 : false,
});

let lastY = 0;
export function onScroll(y: number) {
  const dy = y - lastY;
  lastY = y;
  ui.scrollY = y;
  if (y < 120) ui.minimized = false;
  else if (dy > 6) ui.minimized = true;
  else if (dy < -10) ui.minimized = false;
}

if (typeof window !== "undefined") {
  const mq = matchMedia("(max-width: 759px)");
  mq.addEventListener("change", () => (ui.phone = mq.matches));
}
