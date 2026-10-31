// The screen a component lives on (set by PageView). Lets things that touch
// global state -- the ambient glow, Hue -- act only while their screen is
// the one on top, and take over again when you come back to it.
import { getContext } from "svelte";

export type PageCtx = { index: number; y: number; active: boolean };
export const pageCtx = () => getContext<PageCtx | undefined>("page");
export const isActive = (p: PageCtx | undefined) => (p ? p.active : true);
