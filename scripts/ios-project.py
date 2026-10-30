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
    "UIBackgroundModes": ["audio"],
    "NSAppTransportSecurity": {"NSAllowsArbitraryLoads": True, "NSAllowsLocalNetworking": True},
    "NSLocalNetworkUsageDescription": "Lumen finds your Philips Hue bridge on your network to dim the lights while you watch.",
    "NSBonjourServices": ["_hue._tcp"],
    "LSApplicationQueriesSchemes": ["infuse", "vlc-x-callback", "vlc"],
    "ITSAppUsesNonExemptEncryption": False,
    "UIViewControllerBasedStatusBarAppearance": False,
    "UIStatusBarStyle": "UIStatusBarStyleLightContent",
})

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

yaml.safe_dump(spec, open(spec_path, "w"), sort_keys=False)
print(f"patched {app_name} ({bundle_id}) + LumenWidgets", file=sys.stderr)
