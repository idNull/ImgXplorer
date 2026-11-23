// swift-tools-version:5.5
// La parte de iOS del complemento tauri-plugin-imgx-mobile (Swift). Ver mobile/README_iOS.md.

import PackageDescription

let package = Package(
  name: "tauri-plugin-imgx-mobile",
  platforms: [
    .iOS(.v15)
  ],
  products: [
    .library(
      name: "tauri-plugin-imgx-mobile",
      type: .static,
      targets: ["tauri-plugin-imgx-mobile"])
  ],
  dependencies: [
    .package(name: "Tauri", path: "../.tauri/tauri-api")
  ],
  targets: [
    .target(
      name: "tauri-plugin-imgx-mobile",
      dependencies: [
        .byName(name: "Tauri")
      ],
      path: "Sources")
  ]
)
