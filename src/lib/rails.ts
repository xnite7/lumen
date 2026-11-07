// Every sideways row on a computer: grab it with the mouse and drag, or use
// the arrow tabs at its ends (they show on hover). Rows that draw their own
// arrows mark themselves with data-own-arrows; this adds dragging to those.

const mouse = () => matchMedia("(pointer: fine)").matches;

function arrows(el: HTMLElement) {
  const host = el.parentElement;
  if (!host) return () => {};
  if (getComputedStyle(host).position === "static") host.style.position = "relative";
  const mk = (dir: -1 | 1) => {
    const b = document.createElement("button");
    b.className = `rail-arrow ${dir < 0 ? "l" : "r"}`;
    b.setAttribute("aria-label", dir < 0 ? "Scroll left" : "Scroll right");
    b.innerHTML = `<svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="${dir < 0 ? "M14.5 5.5 8 12l6.5 6.5" : "M9.5 5.5 16 12l-6.5 6.5"}"/></svg>`;
    b.onclick = (e) => {
      e.stopPropagation();
      el.scrollBy({ left: dir * el.clientWidth * 0.82, behavior: "smooth" });
    };
    host.appendChild(b);
    return b;
  };
  const l = mk(-1), r = mk(1);
  const place = () => {
    const top = el.offsetTop, h = el.offsetHeight;
    for (const b of [l, r]) {
      b.style.top = `${top}px`;
      b.style.height = `${h}px`;
    }
    l.classList.toggle("can", el.scrollLeft > 8);
    r.classList.toggle("can", el.scrollLeft + el.clientWidth < el.scrollWidth - 8);
  };
  const enter = () => (place(), host.classList.add("rail-hover"));
  const leave = () => host.classList.remove("rail-hover");
  el.addEventListener("scroll", place, { passive: true });
  host.addEventListener("pointerenter", enter);
  host.addEventListener("pointerleave", leave);
  const ro = new ResizeObserver(place);
  ro.observe(el);
  place();
  return () => {
    l.remove();
    r.remove();
    ro.disconnect();
    el.removeEventListener("scroll", place);
    host.removeEventListener("pointerenter", enter);
    host.removeEventListener("pointerleave", leave);
  };
}

function drag(el: HTMLElement) {
  let x0 = 0, s0 = 0, moved = false, down = false, id = 0;
  const md = (e: PointerEvent) => {
    if (e.pointerType !== "mouse" || e.button !== 0) return;
    down = true;
    moved = false;
    x0 = e.clientX;
    s0 = el.scrollLeft;
    id = e.pointerId;
  };
  const mm = (e: PointerEvent) => {
    if (!down) return;
    const dx = e.clientX - x0;
    if (!moved && Math.abs(dx) > 6) {
      moved = true;
      el.setPointerCapture(id);
      el.classList.add("rail-dragging");
    }
    if (moved) el.scrollLeft = s0 - dx;
  };
  const mu = () => {
    if (!down) return;
    down = false;
    el.classList.remove("rail-dragging");
    if (moved) {
      // a drag isn't a click on whatever it started over
      const stop = (e: Event) => {
        e.stopPropagation();
        e.preventDefault();
      };
      el.addEventListener("click", stop, { capture: true, once: true });
      setTimeout(() => el.removeEventListener("click", stop, { capture: true }), 50);
    }
  };
  el.addEventListener("pointerdown", md);
  el.addEventListener("pointermove", mm);
  el.addEventListener("pointerup", mu);
  el.addEventListener("pointercancel", mu);
  el.addEventListener("dragstart", (e) => moved && e.preventDefault());
}

const seen = new WeakSet<HTMLElement>();
function wire(el: HTMLElement) {
  if (seen.has(el)) return;
  seen.add(el);
  drag(el);
  if (!el.hasAttribute("data-own-arrows")) arrows(el);
}

export function installRails() {
  if (!mouse()) return;
  const scan = (root: ParentNode) => root.querySelectorAll<HTMLElement>(".scroll-x").forEach(wire);
  scan(document);
  new MutationObserver((ms) => {
    for (const m of ms) for (const n of m.addedNodes) if (n instanceof HTMLElement) {
      if (n.classList.contains("scroll-x")) wire(n);
      scan(n);
    }
  }).observe(document.body, { childList: true, subtree: true });
}
