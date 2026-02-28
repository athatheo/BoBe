// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "BoBe",
    platforms: [
        .macOS(.v14)
    ],
    dependencies: [],
    targets: [
        .executableTarget(
            name: "BoBe",
            dependencies: [],
            path: "BoBe",
            exclude: [
                "Resources/Info.plist"
            ],
            resources: [
                .process("Resources")
            ]
        )
    ]
)
