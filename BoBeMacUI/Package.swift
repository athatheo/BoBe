// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "BoBe",
    defaultLocalization: "en",
    platforms: [
        .macOS(.v15)
    ],
    dependencies: [
        .package(url: "https://github.com/sparkle-project/Sparkle", from: "2.9.1"),
        .package(url: "https://github.com/gonzalezreal/textual", from: "0.3.1"),
        // FluidAudio — Apple Silicon ANE Parakeet/Qwen3 ASR + Silero VAD.
        // Mode B client-side speech recognition.
        .package(url: "https://github.com/FluidInference/FluidAudio.git", from: "0.14.5"),
    ],
    targets: [
        .executableTarget(
            name: "BoBe",
            dependencies: [
                .product(name: "Sparkle", package: "Sparkle"),
                .product(name: "Textual", package: "textual"),
                .product(name: "FluidAudio", package: "FluidAudio"),
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
        ),
        .testTarget(
            name: "BoBeTests",
            dependencies: ["BoBe"],
            path: "Tests/BoBeTests"
        )
    ]
)
