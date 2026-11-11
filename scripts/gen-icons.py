#!/usr/bin/env python3
"""Pull the icons couch uses out of Material Symbols (Rounded, Apache-2.0,
via @iconify-json/material-symbols) into src/lib/icons.gen.ts.
Usage: gen-icons.py <path to icons.json>"""
import json, sys

data = json.load(open(sys.argv[1]))
icons, aliases = data["icons"], data.get("aliases", {})

def body(n):
    if n in icons:
        return icons[n]["body"]
    if n in aliases:
        return body(aliases[n]["parent"])
    return None

# couch name -> (outline candidates, filled candidates)
M = {
    "chevron": (["chevron-right-rounded"], []),
    "personCircle": (["account-circle-outline", "account-circle-outline-rounded"], ["account-circle"]),
    "qr": (["qr-code-rounded"], []),
    "key": (["key-outline-rounded", "key-outline"], ["key-rounded"]),
    "rows": (["view-agenda-outline-rounded"], ["view-agenda-rounded"]),
    "stack": (["video-library-outline-rounded"], ["video-library-rounded"]),
    "sliders": (["tune-rounded"], []),
    "storage": (["storage-rounded"], []),
    "wand": (["auto-fix-normal-outline-rounded"], ["auto-fix-normal-rounded"]),
    "share": (["ios-share-rounded"], []),
    "mail": (["mail-outline-rounded"], ["mail-rounded"]),
    "phone": (["smartphone-outline-rounded", "smartphone-outline"], ["smartphone-rounded"]),
    "signout": (["logout-rounded"], []),
    "bookmark": (["bookmark-outline-rounded"], ["bookmark-rounded"]),
    "checkCircle": (["check-circle-outline-rounded"], ["check-circle-rounded"]),
    "hand": (["back-hand-outline-rounded"], ["back-hand-rounded"]),
    "home": (["home-outline-rounded"], ["home-rounded"]),
    "discover": (["explore-outline-rounded"], ["explore-rounded"]),
    "library": (["video-library-outline-rounded"], ["video-library-rounded"]),
    "search": (["search-rounded"], []),
    "settings": (["settings-outline-rounded"], ["settings-rounded"]),
    "addons": (["extension-outline-rounded", "extension-outline"], ["extension-rounded"]),
    "play": (["play-arrow-rounded"], ["play-arrow-rounded"]),
    "pause": (["pause-rounded"], ["pause-rounded"]),
    "plus": (["add-rounded"], []),
    "check": (["check-rounded"], []),
    "close": (["close-rounded"], []),
    "back": (["arrow-back-ios-new-rounded"], []),
    "forward": (["arrow-forward-ios-rounded"], []),
    "chevronDown": (["keyboard-arrow-down-rounded"], []),
    "chevronUp": (["keyboard-arrow-up-rounded"], []),
    "more": (["more-horiz"], []),
    "pencil": (["edit-outline-rounded"], ["edit-rounded"]),
    "heart": (["favorite-outline-rounded"], ["favorite-rounded"]),
    "star": (["star-outline-rounded"], ["star-rounded"]),
    "eye": (["visibility-outline-rounded"], ["visibility-rounded"]),
    "eyeOff": (["visibility-off-outline-rounded"], ["visibility-off-rounded"]),
    "volume": (["volume-up-outline-rounded"], ["volume-up-rounded"]),
    "mute": (["volume-off-outline-rounded"], ["volume-off-rounded"]),
    "subtitles": (["subtitles-outline-rounded", "subtitles-outline"], ["subtitles-rounded"]),
    "audio": (["graphic-eq-rounded"], []),
    "fullscreen": (["fullscreen-rounded"], []),
    "exitFullscreen": (["fullscreen-exit-rounded"], []),
    "pip": (["picture-in-picture-alt-outline-rounded", "picture-in-picture-alt-outline"], ["picture-in-picture-alt-rounded"]),
    "skipB": (["replay-rounded"], []),
    "skipF": (["replay-rounded"], []),
    "replay5": (["replay-5-rounded"], []),
    "replay10": (["replay-10-rounded"], []),
    "replay30": (["replay-30-rounded"], []),
    "forward5": (["forward-5-rounded"], []),
    "forward10": (["forward-10-rounded"], []),
    "forward30": (["forward-30-rounded"], []),
    "next": (["skip-next-rounded"], ["skip-next-rounded"]),
    "skipAhead": (["fast-forward-rounded"], ["fast-forward-rounded"]),
    "trash": (["delete-outline-rounded"], ["delete-rounded"]),
    "refresh": (["refresh-rounded"], []),
    "link": (["link-rounded"], []),
    "sparkles": (["auto-awesome-outline-rounded", "auto-awesome-outline"], ["auto-awesome-rounded"]),
    "bulb": (["lightbulb-outline-rounded"], ["lightbulb-rounded"]),
    "grip": (["drag-indicator"], []),
    "drag": (["drag-handle-rounded"], []),
    "bolt": (["bolt-rounded"], ["bolt-rounded"]),
    "flask": (["science-outline-rounded"], ["science-rounded"]),
    "info": (["info-outline-rounded"], ["info-rounded"]),
    "globe": (["language-rounded"], []),
    "minimize": (["minimize-rounded"], []),
    "maximize": (["crop-square-outline-rounded"], []),
    "film": (["movie-outline-rounded"], ["movie-rounded"]),
    "calendar": (["calendar-month-outline-rounded", "calendar-month-outline"], ["calendar-month-rounded", "calendar-month"]),
    "bell": (["notifications-outline-rounded", "notifications-outline"], ["notifications-rounded", "notifications"]),
    "couch": (["weekend-outline-rounded"], ["weekend-rounded"]),
    "tv": (["tv-outline-rounded"], ["tv-rounded"]),
    "person": (["person-outline-rounded"], ["person-rounded"]),
    "download": (["download-rounded"], []),
    "upload": (["upload-rounded"], []),
    "gauge": (["speed-outline-rounded"], ["speed-rounded"]),
    "magnet": (["attractions-rounded", "hub-outline-rounded"], []),
    "palette": (["palette-outline"], ["palette"]),
}

out, miss = {}, []
for k, (o, f) in M.items():
    ob = next((b for b in map(body, o) if b), None)
    fb = next((b for b in map(body, f) if b), None)
    if not ob:
        miss.append(k)
        continue
    out[k] = [ob] + ([fb] if fb and fb != ob else [])
if miss:
    print("missing:", miss, file=sys.stderr)
with open("src/lib/icons.gen.ts", "w") as fh:
    fh.write("// Generated by scripts/gen-icons.py from Material Symbols Rounded (Apache-2.0).\n")
    fh.write("// name -> [outline svg body, filled svg body?] on a 24px grid\n")
    fh.write("export const ICONS: Record<string, string[]> = " + json.dumps(out, indent=0) + ";\n")
print(len(out), "icons")
