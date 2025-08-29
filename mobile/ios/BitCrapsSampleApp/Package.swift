// swift-tools-version: 5.9
import PackageDescription

let package = Package(
    name: "BitCrapsSampleApp",
    platforms: [
        .iOS(.v15)
    ],
    products: [
        .executable(
            name: "BitCrapsSampleApp",
            targets: ["BitCrapsSampleApp"]
        ),
    ],
    dependencies: [
        .package(path: "../BitCrapsSDK"),
    ],
    targets: [
        .executableTarget(
            name: "BitCrapsSampleApp",
            dependencies: ["BitCrapsSDK"],
            path: "Sources/BitCrapsSampleApp",
            resources: [
                .process("Resources")
            ]
        ),
    ]
)