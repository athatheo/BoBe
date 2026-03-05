// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "BoBe",
    defaultLocalization: "en",
    platforms: [
        .macOS(.v14)
    ],
    dependencies: [
        .package(url: "https://github.com/sparkle-project/Sparkle", from: "2.0.0"),
    ],
    targets: [
        .executableTarget(
            name: "BoBe",
            dependencies: [
                .product(name: "Sparkle", package: "Sparkle"),
            ],
            path: "BoBe",
            exclude: [
                "Resources/Info.plist"
            ],
            resources: [
                .process("Resources")
            ],
            swiftSettings: [
                .define("SPM_BUILD")
            ]
        )
    ]
)
