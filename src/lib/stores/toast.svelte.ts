export interface Toast {
  id: number;
  text: string;
  kind: "info" | "ok" | "warn" | "error";
  action?: { label: string; run: () => void };
}

export const toasts = $state({ list: [] as Toast[] });
let n = 0;

export function toast(text: string, kind: Toast["kind"] = "info", action?: Toast["action"], ms = 3800) {
  const id = ++n;
  toasts.list = [...toasts.list.slice(-3), { id, text, kind, action }];
  setTimeout(() => (toasts.list = toasts.list.filter((t) => t.id !== id)), ms);
}

export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) return String((e as any).message);
  return "Something went wrong";
}
