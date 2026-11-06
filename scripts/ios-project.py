"""Customise the Xcode project `tauri ios init` generates.

Adds what couch's native layer needs: background audio (picture-in-picture
keeps playing), Live Activities, plain-HTTP streams (debrid links, the local
torrent server), local-network access (Hue), the VLC/Infuse URL schemes,
and the widget extension that draws the Dynamic Island.

Run from src-tauri/gen/apple, then `xcodegen generate`.
"""
import sys
import yaml

spec_path = "project.yml"
spec = yaml.safe_load(open(spec_path))
targets = spec["targets"]
app_name = next(n for n, t in targets.items() if t.get("type") == "application")
app = targets[app_name]
bundle_id = app.get("settings", {}).get("base", {}).get("PRODUCT_BUNDLE_IDENTIFIER", "com.midnite.couch")

info = app.setdefault("info", {}).setdefault("properties", {})
info.update({
    "NSSupportsLiveActivities": True,
    "CADisableMinimumFrameDurationOnPhone": True,
    "UIBackgroundModes": ["audio"],
    "NSAppTransportSecurity": {"NSAllowsArbitraryLoads": True, "NSAllowsLocalNetworking": True},
    "NSLocalNetworkUsageDescription": "couch finds your Philips Hue bridge on your network to dim the lights while you watch.",
    "NSBonjourServices": ["_hue._tcp"],
    "LSApplicationQueriesSchemes": ["infuse", "vlc-x-callback", "vlc"],
    "ITSAppUsesNonExemptEncryption": False,
    "UIViewControllerBasedStatusBarAppearance": False,
    "UIStatusBarStyle": "UIStatusBarStyleLightContent",
    # couch:// (Simkl sign-in coming back, links) and stremio:// (addon installs)
    "CFBundleURLTypes": [{"CFBundleURLName": bundle_id, "CFBundleURLSchemes": ["couch", "lumen", "stremio"]}],
})

# couch's own player: mpv through MPVKit (Swift package binaries), plus its
# Swift sources, compiled into the app target.
spec.setdefault("packages", {})["MPVKit"] = {
    "url": "https://github.com/mpvkit/MPVKit",
    "revision": "98c3453e252922e3136422da3fbb2e341ed9edc8",  # 1.1.0-n9.0.2
}
app["deploymentTarget"] = "16.0"  # MPVKit needs 15; EDR + Live Activities want 16
srcs = app.setdefault("sources", [])
if not any((s.get("path") if isinstance(s, dict) else s) == "../../ios-player" for s in srcs):
    srcs.append({"path": "../../ios-player"})
info["UISupportedInterfaceOrientations"] = [
    "UIInterfaceOrientationPortrait",
    "UIInterfaceOrientationLandscapeLeft",
    "UIInterfaceOrientationLandscapeRight",
]

targets["CouchWidgets"] = {
    "type": "app-extension",
    "platform": "iOS",
    "deploymentTarget": "16.2",
    "sources": [{"path": "../../ios-widgets"}],
    "info": {
        "path": "CouchWidgets/Info.plist",
        "properties": {
            "CFBundleDisplayName": "couch",
            "CFBundleShortVersionString": "$(MARKETING_VERSION)",
            "CFBundleVersion": "$(CURRENT_PROJECT_VERSION)",
            "NSExtension": {"NSExtensionPointIdentifier": "com.apple.widgetkit-extension"},
        },
    },
    "settings": {
        "base": {
            "PRODUCT_BUNDLE_IDENTIFIER": f"{bundle_id}.widgets",
            "PRODUCT_NAME": "CouchWidgets",
            "SWIFT_VERSION": "5.0",
            "TARGETED_DEVICE_FAMILY": "1,2",
            "SKIP_INSTALL": "YES",
            "MARKETING_VERSION": app.get("settings", {}).get("base", {}).get("MARKETING_VERSION", "1.0"),
            "CURRENT_PROJECT_VERSION": app.get("settings", {}).get("base", {}).get("CURRENT_PROJECT_VERSION", "1"),
            "ARCHS": "arm64",
        }
    },
}

deps = app.setdefault("dependencies", [])
if not any(d.get("target") == "CouchWidgets" for d in deps):
    deps.append({"target": "CouchWidgets", "embed": True})
if not any(d.get("package") == "MPVKit" for d in deps):
    deps.append({"package": "MPVKit", "product": "MPVKit"})

yaml.safe_dump(spec, open(spec_path, "w"), sort_keys=False)
print(f"patched {app_name} ({bundle_id}) + CouchWidgets", file=sys.stderr)

# The 1024 icon as a single universal entry (iOS draws its own dark/tinted looks).
import glob, json, shutil, os
here = os.path.dirname(os.path.abspath(__file__))
src = os.path.join(here, "..", "src-tauri", "icons", "ios-appearance")
for iconset in glob.glob("**/AppIcon.appiconset", recursive=True):
    cpath = os.path.join(iconset, "Contents.json")
    contents = json.load(open(cpath))
    images = [i for i in contents.get("images", []) if not i.get("appearances") and i.get("filename") != "AppIcon-any.png"]
    for name in ("any",):
        shutil.copy(os.path.join(src, f"AppIcon-{name}.png"), os.path.join(iconset, f"AppIcon-{name}.png"))
    images.append({"filename": "AppIcon-any.png", "idiom": "universal", "platform": "ios", "size": "1024x1024"})
    contents["images"] = images
    json.dump(contents, open(cpath, "w"), indent=2)
    print(f"icon added to {iconset}", file=sys.stderr)
