// swift-tools-version:5.3
import PackageDescription

let package = Package(
    name: "tauri-plugin-lumen-native",
    platforms: [.iOS(.v14)],
    products: [
        .library(name: "tauri-plugin-lumen-native", type: .static, targets: ["tauri-plugin-lumen-native"]),
    ],
    dependencies: [
        .package(name: "Tauri", path: "../.tauri/tauri-api"),
    ],
    targets: [
        .target(
            name: "tauri-plugin-lumen-native",
            dependencies: [.byName(name: "Tauri")],
            path: "Sources"
        ),
    ]
)
