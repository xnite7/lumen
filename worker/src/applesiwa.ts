// Sign in with Apple, the server's side of it: trading the app's one-time
// code for a refresh token, and revoking that token when the account is
// deleted (Apple asks apps to do this). Needs three secrets:
//   APPLE_TEAM_ID       the 10-character team id
//   APPLE_SIWA_KEY_ID   the id of a key with "Sign in with Apple" enabled
//   APPLE_SIWA_P8       that key's .p8 text
// Without them sign-in still works; there's just nothing to revoke.

const CLIENT = "com.midnite.couch";
type E = { APPLE_TEAM_ID?: string; APPLE_SIWA_KEY_ID?: string; APPLE_SIWA_P8?: string };

const b64u = (b: ArrayBuffer | Uint8Array) => btoa(String.fromCharCode(...new Uint8Array(b))).replace(/\+/g, "-").replace(/\//g, "_").replace(/=+$/, "");

/** the "client secret" Apple wants: a short-lived token signed with the key */
async function secret(env: E): Promise<string | null> {
  if (!env.APPLE_TEAM_ID || !env.APPLE_SIWA_KEY_ID || !env.APPLE_SIWA_P8) return null;
  try {
    const der = Uint8Array.from(atob(env.APPLE_SIWA_P8.replace(/-----[A-Z ]+-----/g, "").replace(/\s+/g, "")), (c) => c.charCodeAt(0));
    const key = await crypto.subtle.importKey("pkcs8", der, { name: "ECDSA", namedCurve: "P-256" }, false, ["sign"]);
    const now = Math.floor(Date.now() / 1000);
    const enc = new TextEncoder();
    const head = b64u(enc.encode(JSON.stringify({ alg: "ES256", kid: env.APPLE_SIWA_KEY_ID })));
    const body = b64u(enc.encode(JSON.stringify({ iss: env.APPLE_TEAM_ID, iat: now, exp: now + 300, aud: "https://appleid.apple.com", sub: CLIENT })));
    const sig = await crypto.subtle.sign({ name: "ECDSA", hash: "SHA-256" }, key, enc.encode(`${head}.${body}`));
    return `${head}.${body}.${b64u(sig)}`;
  } catch {
    return null;
  }
}

/** the sign-in's one-time code → a refresh token to keep (null: not set up, or Apple said no) */
export async function appleExchange(env: E, code: string): Promise<string | null> {
  const s = await secret(env);
  if (!s) return null;
  const r = await fetch("https://appleid.apple.com/auth/token", {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({ client_id: CLIENT, client_secret: s, code, grant_type: "authorization_code" }),
  });
  if (!r.ok) return null;
  const v: any = await r.json().catch(() => null);
  return typeof v?.refresh_token === "string" ? v.refresh_token : null;
}

/** the account is going: couch's access to that Apple sign-in ends too */
export async function appleRevoke(env: E, token: string): Promise<boolean> {
  const s = await secret(env);
  if (!s) return false;
  const r = await fetch("https://appleid.apple.com/auth/revoke", {
    method: "POST",
    headers: { "content-type": "application/x-www-form-urlencoded" },
    body: new URLSearchParams({ client_id: CLIENT, client_secret: s, token, token_type_hint: "refresh_token" }),
  });
  return r.ok;
}
