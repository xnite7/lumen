// swift-tools-version:5.3
import PackageDescription

let package = Package(
    name: "tauri-plugin-couch-native",
    platforms: [.iOS(.v14)],
    products: [
        .library(name: "tauri-plugin-couch-native", type: .static, targets: ["tauri-plugin-couch-native"]),
    ],
    dependencies: [
        .package(name: "Tauri", path: "../.tauri/tauri-api"),
    ],
    targets: [
        .target(
            name: "tauri-plugin-couch-native",
            dependencies: [.byName(name: "Tauri")],
            path: "Sources"
        ),
    ]
)
