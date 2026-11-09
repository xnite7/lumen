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
    b.innerHTML = `<span class="disc"><svg width="22" height="22" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="${dir < 0 ? "M14.5 5.5 8 12l6.5 6.5" : "M9.5 5.5 16 12l-6.5 6.5"}"/></svg></span>`;
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

// Mouse drag that feels like a phone: the row follows the hand, keeps
// gliding after you let go and slows down by itself, and at either end it
// stretches a little and springs back ("boing") instead of stopping dead.
function drag(el: HTMLElement) {
  let x0 = 0, s0 = 0, moved = false, down = false, id = 0;
  let lastX = 0, lastT = 0, vel = 0; // px per ms, positive = content moves right
  let glide = 0;
  const max = () => el.scrollWidth - el.clientWidth;
  const stretch = (o: number) => Math.sign(o) * Math.min(90, Math.abs(o) * 0.35);
  const setPull = (px: number, spring = false) => {
    el.style.transition = spring ? "transform 620ms cubic-bezier(0.34, 1.56, 0.64, 1)" : "none";
    el.style.transform = px ? `translateX(${px}px)` : "";
  };
  const stopGlide = () => {
    cancelAnimationFrame(glide);
    glide = 0;
  };

  const md = (e: PointerEvent) => {
    if (e.pointerType !== "mouse" || e.button !== 0) return;
    stopGlide();
    down = true;
    moved = false;
    x0 = lastX = e.clientX;
    lastT = performance.now();
    vel = 0;
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
    if (!moved) return;
    const now = performance.now();
    const dt = Math.max(1, now - lastT);
    // smoothed speed of the hand
    vel = vel * 0.6 + ((e.clientX - lastX) / dt) * 0.4;
    lastX = e.clientX;
    lastT = now;
    const want = s0 - dx;
    const m = max();
    const over = want < 0 ? want : want > m ? want - m : 0;
    el.scrollLeft = Math.min(m, Math.max(0, want));
    setPull(-stretch(over));
  };
  const mu = () => {
    if (!down) return;
    down = false;
    if (!moved) return;
    // a drag isn't a click on whatever it started over
    const stop = (e: Event) => {
      e.stopPropagation();
      e.preventDefault();
    };
    el.addEventListener("click", stop, { capture: true, once: true });
    setTimeout(() => el.removeEventListener("click", stop, { capture: true }), 50);

    // let go while stretched: spring back
    if (el.style.transform) {
      setPull(0, true);
      el.classList.remove("rail-dragging");
      return;
    }
    // released still: nothing to glide
    if (performance.now() - lastT > 80) vel = 0;
    let v = vel;
    let t = performance.now();
    const step = (now: number) => {
      const dt = Math.min(32, now - t);
      t = now;
      const before = el.scrollLeft;
      el.scrollLeft = before - v * dt;
      const m = max();
      // ran into an end while gliding: a little bounce, then rest
      if ((el.scrollLeft <= 0 && v > 0) || (el.scrollLeft >= m && v < 0)) {
        const kick = Math.max(-46, Math.min(46, v * 26));
        setPull(kick);
        requestAnimationFrame(() => setPull(0, true));
        glide = 0;
        el.classList.remove("rail-dragging");
        return;
      }
      v *= Math.pow(0.9955, dt); // friction, frame-rate independent
      if (Math.abs(v) < 0.02) {
        glide = 0;
        el.classList.remove("rail-dragging");
        return;
      }
      glide = requestAnimationFrame(step);
    };
    if (Math.abs(v) > 0.05) glide = requestAnimationFrame(step);
    else el.classList.remove("rail-dragging");
  };
  el.addEventListener("pointerdown", md);
  el.addEventListener("pointermove", mm);
  el.addEventListener("pointerup", mu);
  el.addEventListener("pointercancel", mu);
  el.addEventListener("wheel", stopGlide, { passive: true });
  el.addEventListener("dragstart", (e) => moved && e.preventDefault());
}

const seen = new WeakSet<HTMLElement>();
function wire(el: HTMLElement) {
  if (seen.has(el)) return;
  seen.add(el);
  // with a mouse, rows glide freely instead of clicking into place
  el.classList.add("rail-free");
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
