#!/usr/bin/env python3
"""Put a couch-site folder (index.html, install.html, l/…) into the server's
code, so the website ships with the server and can't overwrite it.
usage: scripts/site-import.py <folder>"""
import glob, os, re, shutil, sys
S = sys.argv[1].rstrip("/")
D = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "worker", "src", "landing")
types = {"jpg": "image/jpeg", "jpeg": "image/jpeg", "png": "image/png", "webp": "image/webp", "woff2": "font/woff2"}
files = sorted(f for f in os.listdir(S + "/l") if f.rsplit(".", 1)[-1] in types)
for f in glob.glob(D + "/*"):
    os.remove(f)
for f in files:
    shutil.copy(f"{S}/l/{f}", f"{D}/{f}")
for name in ["index.html", "install.html"]:
    t = open(f"{S}/{name}").read()
    # absolute addresses: the pages are served at / and /install (and /help, /ios)
    t = re.sub(r'(src|href)="l/', r'\1="/l/', t)
    t = re.sub(r'url\((["\']?)l/', r"url(\1/l/", t)
    t = re.sub(r'(src|href)="couch\.png"', r'\1="/couch.png"', t)  # the couch logo, always
    t = t.replace("https://midnite-couch.xnite7.workers.dev/", "/")
    open(f"{D}/{name}", "w").write(t)
out = ["// The landing page and install guide (designed in Claude Design): the pages plus their pictures and font.", 'import landing from "./index.html";', 'import install from "./install.html";']
out += [f'import a{i} from "./{f}";' for i, f in enumerate(files)]
out += ["", "export const LANDING_HTML: string = landing;", "export const INSTALL_PAGE: string = install;", "export const LANDING_FILES: Record<string, [ArrayBuffer, string]> = {"]
out += [f'  "{f}": [a{i}, "{types[f.rsplit(".", 1)[1]]}"],' for i, f in enumerate(files)]
out.append("};")
open(D + "/landing.ts", "w").write("\n".join(out) + "\n")
print(len(files), "files")
