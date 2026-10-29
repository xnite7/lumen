export function parseIsoLike(s?: string | null): number {
  if (!s) return 0;
  const t = Date.parse(s);
  return isNaN(t) ? 0 : t;
}
