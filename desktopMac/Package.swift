// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "BoBe",
    platforms: [
        .macOS(.v14)
    ],
    targets: [
        .executableTarget(
            name: "BoBe",
            path: "BoBe",
            resources: [
                .process("Resources")
            ]
        )
    ]
)
