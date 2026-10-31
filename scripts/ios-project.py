"""Customise the Xcode project `tauri ios init` generates.

Adds what Lumen's native layer needs: background audio (picture-in-picture
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
bundle_id = app.get("settings", {}).get("base", {}).get("PRODUCT_BUNDLE_IDENTIFIER", "app.lumen.player")

info = app.setdefault("info", {}).setdefault("properties", {})
info.update({
    "NSSupportsLiveActivities": True,
    "CADisableMinimumFrameDurationOnPhone": True,
    "UIBackgroundModes": ["audio"],
    "NSAppTransportSecurity": {"NSAllowsArbitraryLoads": True, "NSAllowsLocalNetworking": True},
    "NSLocalNetworkUsageDescription": "Lumen finds your Philips Hue bridge on your network to dim the lights while you watch.",
    "NSBonjourServices": ["_hue._tcp"],
    "LSApplicationQueriesSchemes": ["infuse", "vlc-x-callback", "vlc"],
    "ITSAppUsesNonExemptEncryption": False,
    "UIViewControllerBasedStatusBarAppearance": False,
    "UIStatusBarStyle": "UIStatusBarStyleLightContent",
    # lumen:// (Simkl sign-in coming back, links) and stremio:// (addon installs)
    "CFBundleURLTypes": [{"CFBundleURLName": bundle_id, "CFBundleURLSchemes": ["lumen", "stremio"]}],
})

# Lumen's own player: mpv through MPVKit (Swift package binaries), plus its
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

targets["LumenWidgets"] = {
    "type": "app-extension",
    "platform": "iOS",
    "deploymentTarget": "16.2",
    "sources": [{"path": "../../ios-widgets"}],
    "info": {
        "path": "LumenWidgets/Info.plist",
        "properties": {
            "CFBundleDisplayName": "Lumen",
            "CFBundleShortVersionString": "$(MARKETING_VERSION)",
            "CFBundleVersion": "$(CURRENT_PROJECT_VERSION)",
            "NSExtension": {"NSExtensionPointIdentifier": "com.apple.widgetkit-extension"},
        },
    },
    "settings": {
        "base": {
            "PRODUCT_BUNDLE_IDENTIFIER": f"{bundle_id}.widgets",
            "PRODUCT_NAME": "LumenWidgets",
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
if not any(d.get("target") == "LumenWidgets" for d in deps):
    deps.append({"target": "LumenWidgets", "embed": True})
if not any(d.get("package") == "MPVKit" for d in deps):
    deps.append({"package": "MPVKit", "product": "MPVKit"})

yaml.safe_dump(spec, open(spec_path, "w"), sort_keys=False)
print(f"patched {app_name} ({bundle_id}) + LumenWidgets", file=sys.stderr)
