// Lift an element to <body>, out of its screen's layer, so overlays (sheets)
// stack above the tab bar no matter which screen opened them.
export function portal(node: HTMLElement) {
  document.body.appendChild(node);
  return { destroy: () => node.remove() };
}
