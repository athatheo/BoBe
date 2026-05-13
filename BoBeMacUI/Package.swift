// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "BoBe",
    defaultLocalization: "en",
    platforms: [
        .macOS(.v15)
    ],
    dependencies: [
        .package(url: "https://github.com/sparkle-project/Sparkle", from: "2.9.0"),
        // Pinned by revision — alta/swift-opus has no tagged releases (last
        // release v0.0.2 Feb 2022, no git tag). Anchor by SHA for reproducible
        // builds; bump intentionally when re-evaluated.
        .package(
            url: "https://github.com/alta/swift-opus",
            revision: "6f3cb6bd3ffed1fe5f06d00a962d5c191a50daf8"
        ),
        .package(url: "https://github.com/gonzalezreal/textual", from: "0.3.1"),
    ],
    targets: [
        .executableTarget(
            name: "BoBe",
            dependencies: [
                .product(name: "Sparkle", package: "Sparkle"),
                .product(name: "Opus", package: "swift-opus"),
                .product(name: "Textual", package: "textual"),
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
