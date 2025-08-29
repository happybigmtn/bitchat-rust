// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "BitCrapsSDK",
    platforms: [
        .iOS(.v14),
        .macOS(.v11)
    ],
    products: [
        .library(
            name: "BitCrapsSDK",
            targets: ["BitCrapsSDK"]
        ),
    ],
    dependencies: [
        // No external dependencies - pure Swift implementation
    ],
    targets: [
        .target(
            name: "BitCrapsSDK",
            dependencies: [],
            path: "Sources/BitCrapsSDK",
            resources: [
                .process("Resources")
            ]
        ),
        .testTarget(
            name: "BitCrapsSDKTests",
            dependencies: ["BitCrapsSDK"],
            path: "Tests/BitCrapsSDKTests"
        ),
    ]
)