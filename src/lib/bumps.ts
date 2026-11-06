// Adult Swim-style bumps: a line of white text on black between things.
// The lines live in src/assets/bumps.json so they can be edited by hand.

import pool from "../assets/bumps.json";

type Pool = "launch" | "late" | "idle";

const usable = (xs: string[]) => xs.filter((x) => x && !x.startsWith("TODO"));
const recent: string[] = [];

/** One line for the moment, never repeating the last few. */
export function bumpLine(kind: Pool, d = new Date()): string {
  const h = d.getHours();
  const late = h >= 1 && h < 5;
  let lines = usable(pool[kind]);
  if (late && kind !== "idle") lines = usable(pool.late);
  lines = [...lines, ...usable(pool.midnite)];
  const fresh = lines.filter((l) => !recent.includes(l));
  const pick = (fresh.length ? fresh : lines)[Math.floor(Math.random() * (fresh.length || lines.length))];
  recent.push(pick);
  if (recent.length > 6) recent.shift();
  return pick;
}
