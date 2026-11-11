# Paper, wax and ticket art for the GPUI watch-party invite (rendered once,
# baked into the app). Run: python3 -I tools/party_assets.py
import numpy as np
from PIL import Image, ImageDraw, ImageFilter, ImageChops
import os, sys
OUT = os.path.join(os.path.dirname(__file__), "..", "desktop", "assets", "party")
S = 2  # pixel density
W, H = 620 * S, 388 * S
rng = np.random.default_rng(7)

def paper(w, h, c1=(239, 228, 207), c2=(227, 212, 184), grain=0.10):
    y = np.linspace(0, 1, h)[:, None]
    x = np.linspace(0, 1, w)[None, :]
    t = np.clip(0.55 * y + 0.45 * x, 0, 1)
    base = np.zeros((h, w, 3))
    for i in range(3):
        base[..., i] = c1[i] * (1 - t) + c2[i] * t
    n = rng.normal(0, 1, (h, w))
    n = np.array(Image.fromarray(((n * 40) + 128).clip(0, 255).astype(np.uint8)).filter(ImageFilter.GaussianBlur(0.7)), dtype=float) - 128
    fib = rng.normal(0, 1, (h, w // 6 + 1)).repeat(6, axis=1)[:, :w]
    base += (n * grain * 2.2)[..., None] * np.array([1.0, 0.85, 0.65])
    base += (fib * 2.0)[..., None]
    return Image.fromarray(base.clip(0, 255).astype(np.uint8), "RGB").convert("RGBA")

def rounded_mask(w, h, r):
    m = Image.new("L", (w, h), 0)
    ImageDraw.Draw(m).rounded_rectangle((0, 0, w - 1, h - 1), r, fill=255)
    return m

def poly_mask(w, h, pts):
    m = Image.new("L", (w, h), 0)
    ImageDraw.Draw(m).polygon([(x * w, y * h) for x, y in pts], fill=255)
    return m

# envelope back (the inside you see when the flap is open: a bit darker)
back = paper(W, H, (222, 208, 180), (206, 189, 158))
back.putalpha(rounded_mask(W, H, 10 * S))
back.save(f"{OUT}/back.png")

# front pocket: two side folds + bottom fold, creases, a soft shadow on top edge
front = paper(W, H)
mask = poly_mask(W, H, [(0, 0), (0.5, 0.52), (1, 0), (1, 1), (0, 1)])
mask = ImageChops.multiply(mask, rounded_mask(W, H, 10 * S))
d = ImageDraw.Draw(front)
for a, b in [((0, 1), (0.5, 0.52)), ((1, 1), (0.5, 0.52))]:
    d.line([(a[0] * W, a[1] * H), (b[0] * W, b[1] * H)], fill=(150, 120, 85, 70), width=2 * S)
front.putalpha(mask)
sh = Image.new("RGBA", (W, H), (0, 0, 0, 0))
sh.paste((60, 30, 10, 90), (0, 0), mask)
sh = sh.filter(ImageFilter.GaussianBlur(5 * S))
sh = ImageChops.offset(sh, 0, -3 * S)
out = Image.alpha_composite(sh, front)
out.save(f"{OUT}/front.png")

# flap, closed (pointing down over the pocket) and open (pointing up)
fh = int(H * 0.56)
closed = paper(W, fh, (243, 233, 214), (231, 217, 190))
closed.putalpha(poly_mask(W, fh, [(0, 0), (1, 0), (0.5, 1)]))
d = ImageDraw.Draw(closed)
d.line([(0, 0), (W / 2, fh)], fill=(140, 110, 80, 60), width=S)
d.line([(W, 0), (W / 2, fh)], fill=(140, 110, 80, 60), width=S)
shc = Image.new("RGBA", (W, fh + 12 * S), (0, 0, 0, 0))
shc.paste((50, 25, 5, 110), (0, 6 * S), poly_mask(W, fh, [(0, 0), (1, 0), (0.5, 1)]))
shc = shc.filter(ImageFilter.GaussianBlur(5 * S))
shc.alpha_composite(closed, (0, 0))
shc.save(f"{OUT}/flap-closed.png")
opened = paper(W, fh, (214, 199, 170), (200, 182, 150))
opened.putalpha(poly_mask(W, fh, [(0, 1), (1, 1), (0.5, 0)]))
opened.save(f"{OUT}/flap-open.png")

# wax seal, cracked into two halves (each half on its own canvas)
ss = 112 * S
wax = np.zeros((ss, ss, 4))
yy, xx = np.mgrid[0:ss, 0:ss] / ss
r = np.hypot(xx - 0.5, (yy - 0.5) * 1.02)
wobble = 0.47 + 0.018 * np.sin(np.arctan2(yy - 0.5, xx - 0.5) * 7) + 0.01 * np.sin(np.arctan2(yy - 0.5, xx - 0.5) * 13 + 1)
inside = r < wobble
t = np.clip(r / 0.5, 0, 1)
col = np.array([224, 48, 138]) * (1 - t[..., None] ** 1.6) + np.array([109, 11, 58]) * (t[..., None] ** 1.6)
hl = np.exp(-(((xx - 0.36) ** 2) + ((yy - 0.3) ** 2)) / 0.012) * 90
col = col + hl[..., None]
rim = np.clip(1 - np.abs(r - 0.33) / 0.02, 0, 1) * 30  # pressed ring
col = col - rim[..., None]
wax[..., :3] = col.clip(0, 255)
wax[..., 3] = inside * 255
waxi = Image.fromarray(wax.astype(np.uint8), "RGBA").filter(ImageFilter.GaussianBlur(0.6))
mark = Image.open(os.path.join(os.path.dirname(__file__), "..", "src", "assets", "brand", "couch-mark.png")).convert("RGBA")
mw = int(ss * 0.56)
mark = mark.resize((mw, int(mark.height * mw / mark.width)), Image.LANCZOS)
ma = mark.split()[3]
pos = ((ss - mark.width) // 2, (ss - mark.height) // 2)
dark = Image.new("RGBA", mark.size, (80, 4, 40, 230)); dark.putalpha(ma.point(lambda v: v * 0.85))
light = Image.new("RGBA", mark.size, (255, 170, 215, 150)); light.putalpha(ma.point(lambda v: v * 0.55))
waxi.alpha_composite(light, (pos[0], pos[1] + S))
waxi.alpha_composite(dark, pos)
# drop shadow
canvas = Image.new("RGBA", (ss + 20 * S, ss + 20 * S), (0, 0, 0, 0))
shadow = Image.new("RGBA", canvas.size, (0, 0, 0, 0))
shadow.paste((60, 0, 30, 150), (10 * S, 16 * S), waxi.split()[3])
shadow = shadow.filter(ImageFilter.GaussianBlur(5 * S))
canvas.alpha_composite(shadow)
canvas.alpha_composite(waxi, (10 * S, 10 * S))
cw, ch = canvas.size
crack = [(0.54, 0), (0.44, 0.34), (0.58, 0.52), (0.46, 0.7), (0.52, 1)]
left = poly_mask(cw, ch, [(0, 0)] + crack + [(0, 1)])
right = poly_mask(cw, ch, [(1, 0)] + crack + [(1, 1)])
for name, m in (("seal-l", left), ("seal-r", right)):
    half = canvas.copy()
    half.putalpha(ImageChops.multiply(canvas.split()[3], m))
    half.save(f"{OUT}/{name}.png")

# the ticket: cream card, inner border lines, notches at the tear line, dashed tear
TW, TH = int(W * 0.9), int(H * 0.82)
tk = paper(TW, TH, (251, 243, 227), (239, 225, 199), grain=0.07)
d = ImageDraw.Draw(tk)
d.rounded_rectangle((6 * S, 6 * S, TW - 6 * S, TH - 6 * S), 4 * S, outline=(196, 32, 111, 95), width=S)
tear = int(TW * 0.765)
for y in range(0, TH, 9 * S):
    d.line([(tear, y), (tear, y + 4 * S)], fill=(59, 16, 41, 90), width=S)
m = rounded_mask(TW, TH, 6 * S)
md = ImageDraw.Draw(m)
nr = 11 * S
md.ellipse((tear - nr, -nr, tear + nr, nr), fill=0)
md.ellipse((tear - nr, TH - nr, tear + nr, TH + nr), fill=0)
tk.putalpha(m)
tk.save(f"{OUT}/ticket.png")

# postmark: two rings and wavy cancel lines
pm = Image.new("RGBA", (120 * S, 70 * S), (0, 0, 0, 0))
d = ImageDraw.Draw(pm)
ink = (59, 16, 41, 120)
d.ellipse((8 * S, 8 * S, 62 * S, 62 * S), outline=ink, width=int(1.6 * S))
d.ellipse((14 * S, 14 * S, 56 * S, 56 * S), outline=ink, width=S)
import math
for k in range(3):
    y0 = (22 + 13 * k) * S
    pts = [(66 * S + i, y0 + math.sin(i / (9 * S) * math.pi) * 3 * S) for i in range(0, 54 * S)]
    d.line(pts, fill=ink, width=int(1.6 * S))
pm.save(f"{OUT}/postmark.png")
# the big ones as WebP (noise compresses badly as PNG)
for f in ("back", "front", "flap-closed", "flap-open", "ticket"):
    im = Image.open(f"{OUT}/{f}.png"); im.save(f"{OUT}/{f}.webp", "WEBP", quality=86, method=6); os.remove(f"{OUT}/{f}.png")
print("ok")
