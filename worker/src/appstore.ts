// couch Pro bought in the App Store. The app sends StoreKit 2's signed
// transaction (a JWS); this checks Apple signed it, all the way to Apple's
// root certificate, and that it's an unrevoked, unexpired couch Pro purchase.
// Returns the purchase's originalTransactionId (stable per buyer), or null.

const BUNDLE = "com.midnite.couch";
const PRODUCT_PREFIX = "com.midnite.couch.pro";
const ROOT_URL = "https://www.apple.com/certificateauthority/AppleRootCA-G3.cer";

// Apple's marker extensions: leaf = App Store receipt signing,
// intermediate = Apple Worldwide Developer Relations
const LEAF_OID = [0x06, 0x0a, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x63, 0x64, 0x06, 0x0b, 0x01];
const MID_OID = [0x06, 0x0a, 0x2a, 0x86, 0x48, 0x86, 0xf7, 0x63, 0x64, 0x06, 0x02, 0x01];

let root: Uint8Array | null = null;

async function appleRoot(): Promise<Uint8Array | null> {
  if (root) return root;
  const r = await fetch(ROOT_URL, { cf: { cacheTtl: 86400 * 30, cacheEverything: true } } as any);
  if (!r.ok) return null;
  root = new Uint8Array(await r.arrayBuffer());
  return root;
}

const b64 = (s: string) => Uint8Array.from(atob(s.replace(/-/g, "+").replace(/_/g, "/")), (c) => c.charCodeAt(0));

// ---- the bit of DER needed: walk TLVs
type Tlv = { tag: number; start: number; body: number; end: number };
function tlv(b: Uint8Array, at: number): Tlv {
  const tag = b[at];
  let len = b[at + 1];
  let body = at + 2;
  if (len & 0x80) {
    const n = len & 0x7f;
    len = 0;
    for (let i = 0; i < n; i++) len = len * 256 + b[at + 2 + i];
    body = at + 2 + n;
  }
  return { tag, start: at, body, end: body + len };
}
function kids(b: Uint8Array, t: Tlv): Tlv[] {
  const out: Tlv[] = [];
  for (let at = t.body; at < t.end; ) {
    const k = tlv(b, at);
    out.push(k);
    at = k.end;
  }
  return out;
}
const has = (b: Uint8Array, pat: number[]) => {
  outer: for (let i = 0; i + pat.length <= b.length; i++) {
    for (let j = 0; j < pat.length; j++) if (b[i + j] !== pat[j]) continue outer;
    return true;
  }
  return false;
};
const same = (a: Uint8Array, b: Uint8Array) => a.length === b.length && a.every((v, i) => v === b[i]);

type Cert = { der: Uint8Array; tbs: Uint8Array; spki: Uint8Array; sigHash: string; sig: Uint8Array };
function parse(der: Uint8Array): Cert {
  const top = tlv(der, 0);
  const [tbsT, algT, sigT] = kids(der, top);
  const tk = kids(der, tbsT);
  const spkiT = tk[tk[0].tag === 0xa0 ? 6 : 5];
  const alg = der.slice(algT.start, algT.end);
  // ecdsa-with-SHA256 ends …04 03 02, SHA384 …04 03 03
  const sigHash = has(alg, [0x04, 0x03, 0x03]) ? "SHA-384" : "SHA-256";
  return {
    der,
    tbs: der.slice(tbsT.start, tbsT.end),
    spki: der.slice(spkiT.start, spkiT.end),
    sigHash,
    sig: der.slice(sigT.body + 1, sigT.end), // skip the BIT STRING's unused-bits byte
  };
}
const curve = (spki: Uint8Array) => (has(spki, [0x2b, 0x81, 0x04, 0x00, 0x22]) ? { c: "P-384", n: 48 } : { c: "P-256", n: 32 });

/** DER ECDSA signature (SEQUENCE { r, s }) → raw r‖s, as WebCrypto wants */
function raw(sig: Uint8Array, n: number): Uint8Array {
  const [r, s] = kids(sig, tlv(sig, 0));
  const fix = (t: Tlv) => {
    let v = sig.slice(t.body, t.end);
    while (v.length > n && v[0] === 0) v = v.slice(1);
    const o = new Uint8Array(n);
    o.set(v, n - v.length);
    return o;
  };
  const o = new Uint8Array(n * 2);
  o.set(fix(r), 0);
  o.set(fix(s), n);
  return o;
}

async function key(spki: Uint8Array) {
  const { c } = curve(spki);
  return crypto.subtle.importKey("spki", spki, { name: "ECDSA", namedCurve: c }, false, ["verify"]);
}

/** child was signed by parent's key */
async function signedBy(child: Cert, parent: Cert) {
  const { n } = curve(parent.spki);
  return crypto.subtle.verify({ name: "ECDSA", hash: child.sigHash }, await key(parent.spki), raw(child.sig, n), child.tbs);
}

export async function appStorePro(jws: string): Promise<string | null> {
  try {
    const [h, p, s] = jws.split(".");
    const head = JSON.parse(new TextDecoder().decode(b64(h)));
    if (head.alg !== "ES256" || !Array.isArray(head.x5c) || head.x5c.length !== 3) return null;
    const [leaf, mid, top] = head.x5c.map((c: string) => parse(b64(c)));
    const apple = await appleRoot();
    if (!apple || !same(top.der, apple)) return null;
    if (!has(leaf.der, LEAF_OID) || !has(mid.der, MID_OID)) return null;
    if (!(await signedBy(mid, top)) || !(await signedBy(leaf, mid))) return null;
    const ok = await crypto.subtle.verify({ name: "ECDSA", hash: "SHA-256" }, await key(leaf.spki), b64(s), new TextEncoder().encode(`${h}.${p}`));
    if (!ok) return null;
    const t = JSON.parse(new TextDecoder().decode(b64(p)));
    if (t.bundleId !== BUNDLE || !String(t.productId ?? "").startsWith(PRODUCT_PREFIX)) return null;
    if (t.revocationDate) return null;
    if (t.expiresDate && t.expiresDate < Date.now()) return null;
    return String(t.originalTransactionId ?? t.transactionId ?? "") || null;
  } catch {
    return null;
  }
}
