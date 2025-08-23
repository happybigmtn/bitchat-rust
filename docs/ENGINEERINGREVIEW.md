23 August 2025 Engineering Review
BitCraps Codebase Review and Mobile Expansion Plan
✅ Recent Improvements and Architecture Overview
The BitCraps repository has evolved significantly, incorporating previous feedback into a robust, fully-functional decentralized casino protocol. All critical components – Bluetooth mesh networking, peer discovery, Noise-protocol encryption, consensus, and token economy – are now implemented and integrated
GitHub
. The architecture remains modular and well-organized, with clear separation of concerns: e.g. transport for BLE communication, protocol/consensus for game state agreement, crypto for keys/signatures, token for CRAP token ledger, and ui for the CLI/TUI interface
GitHub
. This modularity makes the codebase maintainable and positions it well for cross-platform expansion. Key architectural points and improvements include:
Bluetooth Mesh Networking: The BluetoothTransport uses the btleplug crate to handle BLE advertising, scanning, and GATT communication across platforms
GitHub
GitHub
. It defines a custom GATT service (UUID 12345678-1234-5678-1234-567812345678) with TX/RX characteristics for bidirectional messaging
GitHub
. The transport code implements connection management, fragmenting large packets to fit BLE MTU, and reassembling them with timeouts for reliability
GitHub
GitHub
. The engineers also introduced a memory pool for packet buffers to reduce allocations
GitHub
. (One observation: the memory pool uses a global mutex-protected buffer list, which could become a contention bottleneck under high concurrency
GitHub
. In future, replacing this with a lock-free structure or per-connection buffer pools – e.g. using a concurrent queue or DashMap – would improve throughput.) Overall, the transport layer is comprehensive, handling multi-peer connectivity, reconnections, and rate limiting of connection attempts to prevent abuse
GitHub
GitHub
.
Peer Discovery and DHT: On top of BLE, BitCraps implements local peer discovery and a basic distributed hash table. The discovery module broadcasts peer identities and maintains a registry with TTL to drop inactive peers
GitHub
. Additionally, a Kademlia-like DHT is outlined for routing and value storage across the mesh
GitHub
. This means the system can scale beyond direct radio range by relaying through peers (the “Proof-of-Relay” mechanism). The recent update indicates the Kademlia DHT is now working
GitHub
. This is a crucial distributed systems component to achieve a true mesh network of games. Going forward, thorough testing of the DHT in multi-hop scenarios will be important, as well as optimizing for low-power devices (Kademlia’s queries and buckets should be tuned for the small network sizes expected over BLE).
Game Logic and Consensus: The craps game mechanics and fairness guarantees are handled through a custom consensus engine. Each game is essentially a mini blockchain: players collectively agree on state updates (bets placed, dice outcomes, payouts) via proposals and votes
GitHub
GitHub
. The consensus engine uses a combination of techniques: a PBFT-style vote tracking for state changes, commit-reveal scheme for dice rolls, and even fork resolution logic for conflicting states
GitHub
GitHub
. Notably, for randomness fairness, the code implements a commit–reveal protocol: each player commits a hash of a random nonce, then later reveals the nonce so that all nonces are combined for the dice result
GitHub
GitHub
. This prevents any single player from biasing the roll. The commitment and reveal messages include digital signatures and are verified on receipt
GitHub
GitHub
. In the latest code, the previously missing signature checks have been added – for example, rekey messages and randomness commits are now properly signed with Ed25519 and verified against the sender’s public key
GitHub
(resolving a former critical vulnerability where signature verification was a no-op
GitHub
). The consensus state is stored in a struct (GameConsensusState) that includes the game phase, all player balances, and a state hash
GitHub
GitHub
. Impressively, the team introduced copy-on-write using Arc to avoid expensive deep copies of game state on each update
GitHub
GitHub
. The current_state is an Arc, so when modifying state for a new block, only the diff is applied, dramatically improving performance by preventing 1000x slowdowns due to cloning large state objects
GitHub
. This attention to consensus performance and integrity is a strong indicator of production readiness. For continued development, it would be wise to conduct more chaos testing of consensus (simulate dropped messages, malicious actors sending invalid data, network partitions) to ensure the fork and dispute mechanisms work as intended. The code has placeholders for dispute resolution votes and byzantine fault tolerance thresholds
GitHub
GitHub
– fleshing those out and testing with >4 nodes (to allow 1/3 faulty) would be a valuable next step.
Cryptography and Security: All cryptographic operations leverage well-vetted libraries, which is a great practice
GitHub
. The code uses ed25519-dalek for Ed25519 keypairs and signatures, x25519 (Noise protocol via snow or similar) for Diffie-Hellman key exchange, and ChaCha20-Poly1305 for authenticated encryption of session data
GitHub
GitHub
. The Noise Protocol framework underpins secure P2P sessions, providing confidentiality and forward secrecy. Crucially, the forward secrecy key rotation is implemented: the ForwardSecrecyManager periodically generates new ephemeral keypairs and securely erases old keys from memory using zeroize
GitHub
GitHub
. This was a high-priority fix from the last review, and it’s been addressed – old session keys are zeroed out to prevent long-term compromise
GitHub
. The rekey flow now signs the new key’s public part and verifies it on the receiving side
GitHub
GitHub
, plugging the prior verification bypass
GitHub
. Another security improvement is the introduction of bounded caches and rate-limiting: the code uses LRU caches and token-bucket style limits to prevent memory exhaustion or denial-of-service from too many messages
GitHub
. For example, a multi-tier cache system provides an in-memory L1 with DashMap (fast, lock-free reads) and an L2 with an LRU for overflow
GitHub
GitHub
– this ensures even if the network is spammed, old entries get evicted and memory use stays bounded. There’s also mention of a message queue with priority and depth monitoring
GitHub
, which would help in scenarios where critical messages (like consensus votes) shouldn’t be starved by less important chatter. All these measures indicate a strong security mindset. Going forward, a formal security audit and fuzz testing of the protocol (especially custom serialization/deserialization code) would be prudent. For instance, the commit/reveal structs use manual serialization of fields
GitHub
GitHub
– we should ensure no unbounded data can cause buffer overflows or panics. But overall, the use of proven crypto, proper validation, and memory safety features (Rust’s ownership plus explicit zeroization) makes the security posture strong.
Token Economy (CRAP) and Mining: The CRAP token functionality is implemented to reward players who relay messages (“Proof-of-Relay”). Every message forwarded through a node can earn it some tokens, incentivizing participation in the mesh
GitHub
GitHub
. The token module likely keeps track of balances and mines new tokens for relayers – e.g., there might be a counter of hops a message took, used to distribute rewards. The exact mechanism wasn’t detailed in the code excerpts, but we see that player balances are tracked in consensus state
GitHub
and updated via proposals (GameOperation variants for balance updates)
GitHub
. The recent commits connected the relay events to the token ledger and included mining difficulty adjustment and reward calculation logic
GitHub
. This means as the network grows or traffic increases, the reward scheme can adjust to control token inflation. One area to extend in future is token persistence: currently, token balances exist in memory and within the game session. It would be useful to persist them to a ledger file or database on each device, so that a user’s earnings can carry over between sessions (or integrate with a blockchain if desired). The code does mention a persistent_ledger.rs module, suggesting an on-disk storage of token transactions is planned or implemented. Ensuring this ledger is crash-safe and perhaps auditable (so that players can’t tamper with their own balance store) is a good next development step if full real-world use is envisioned.
Performance and Efficiency: The engineering team has clearly focused on performance optimizations, which is crucial given the constraints of BLE bandwidth and mobile devices. The inclusion of SIMD-accelerated crypto operations and adaptive LZ4/Zlib compression for messages is notable
GitHub
GitHub
. Compression can shrink messages by 60–80% on average
GitHub
, which is extremely valuable over BLE (which tops out at a few hundred kbps). The code’s MTU adaptive fragmentation tries to send the largest possible BLE packets by discovering the peer’s MTU and adjusting message size accordingly
GitHub
GitHub
. Larger MTUs mean fewer fragments per message, reducing overhead. They also implemented connection pooling and quality-based connection management
GitHub
, so the system can maintain up to 10× more connections by reusing them and prioritizing stable links. On the consensus side, the lock-free design and caching yield massive speedups: according to internal benchmarks, consensus latency dropped by 100×–1000× after making it lock-free and caching signatures
GitHub
. The Efficiency Report documents remarkable gains – e.g. 50× faster bet resolution using lookup tables, 90%+ memory reduction via bit-packing
GitHub
GitHub
. These numbers are backed by the benchmark suite in benches/performance.rs, which tests key operations like signature verification, packet serialization, compression, and cache hits under load
GitHub
GitHub
. For instance, the benchmarks show Ed25519 signature verification throughput improved 4× with batch processing and caching
GitHub
. Another critical optimization implemented is Arc-based signature caching – repeated verifications of the same data are avoided via an LRU cache
GitHub
, important because in a mesh, multiple peers might send you the same proposal or block to verify. Overall, the codebase demonstrates excellent performance engineering: custom memory pools, multi-tier caches, and avoiding clones in hot paths
GitHub
. Future performance work can focus on battery optimization (important for mobile): e.g., dynamically scaling BLE scanning/advertising intervals when the network is quiet to save power
GitHub
, and perhaps using OS-specific low-energy BLE modes. Additionally, as more features accumulate, continuous profiling (the project already has Criterion benchmarks) should be part of CI to catch any regressions early. But at this stage, performance looks very promising, with headroom to support “thousands of concurrent games” as claimed
GitHub
– likely theoretical, but it shows the architecture can scale.
Code Quality and Maintainability: The codebase is written in idiomatic Rust with async/await (Tokio runtime) for concurrency, which is appropriate for a networking-heavy application
GitHub
. Error handling is centralized in a custom Error type (Result<T> throughout), making it easier to manage failures gracefully. The team has also significantly improved code quality by cleaning up dead code and warnings (as noted in recent commits) and refactoring large modules into smaller ones
GitHub
. For example, the previously 1,100+ line kademlia.rs might have been split into mesh/kademlia_dht.rs and discovery/dht_discovery.rs, and similarly the consensus code was modularized (there’s now protocol/consensus/engine.rs, commit_reveal.rs, etc., instead of one giant file). This was a recommended change to keep modules under ~500 lines for readability
GitHub
. The internal documentation is thorough – nearly every module has a top-level doc comment explaining its purpose, and an extensive docs/ folder exists with plans and reports. The last senior review noted 2,000+ lines of function docs and 100+ tests
GitHub
, which is an excellent indicator of maintainability. The tests cover critical functionality (there are integration tests simulating end-to-end game flow
GitHub
and benchmarks doubling as stress tests). One area to continue improving is unit test coverage for edge cases – e.g., tests for signature verification (valid vs invalid cases)
GitHub
, tests for consensus fork resolution, etc., to prevent regressions in those complex algorithms. Also, as development continues, the team should monitor module size and complexity; e.g., transport/bluetooth.rs is still ~1000+ lines
GitHub
– breaking it into sub-modules (perhaps separating fragmentation, scanning, and connection handling logic) could make it easier for new contributors to navigate.
Positive Findings: Overall, the updated BitCraps codebase reflects strong engineering practices and significant progress towards production readiness. The critical security issues from earlier reviews have been fixed (e.g. proper signature validation is now in place)
GitHub
, and numerous performance and architectural enhancements have been added. The system is now 100% functional with all planned features implemented
GitHub
. As a distributed systems and cryptography expert, I’m pleased to see the use of established cryptographic libraries (avoiding custom insecure algorithms)
GitHub
, the careful handling of key material, and the thoughtful design of a consensus mechanism tailored to the use case. The next sections outline recommended areas for ongoing development and how to extend this Rust-based framework to mobile platforms for full feature parity.
🔭 Recommended Areas for Continued Development
With the core functionality in place, the team should focus on hardening the system and preparing for real-world deployment. Here are the key areas for continued development:
Rigorous Testing & QA: Expand the test suite to include multi-device integration tests and simulations of various conditions. For example, test with 5+ nodes to observe consensus behavior under slight network delays or packet loss. Include property-based tests or fuzz tests for packet encoding/decoding and state transitions, to catch edge-case bugs (especially in consensus and DHT logic). Testing recovery from network partitions (split brain scenarios) is important – ensure that when the mesh reconnects, game state convergence is handled correctly
GitHub
GitHub
. Moreover, testing on actual hardware (Android and iOS devices with Bluetooth) will be crucial given the complexity of BLE and varying platform constraints.
Security Audits & Hardening: Although the cryptography approach is solid, a dedicated security audit should be done. This includes a threat modeling exercise for cheating or abuse scenarios: e.g., can a malicious node collude or replay old messages to gain tokens illegitimately? The code uses a message cache and signatures to mitigate this, but an audit can verify there are no logic flaws. Implementing pen tests or bug bounties for the protocol would be wise before real stakes are involved. Additionally, consider hardening the cryptography by using constant-time comparisons where appropriate (Rust’s subtle crate is used for equality checks
GitHub
which is good). Another area is key management: currently identities are generated on the fly. For a production deployment, you may want to allow key import/export or device pairing mechanisms so users can reuse an identity (with appropriate safeguards).
Battery and Performance Optimizations: Running a Bluetooth mesh can be power-intensive on mobile devices. Investigate strategies like adaptive scanning – e.g., slow down scan frequency when few peers are around, and increase it when a new game is being formed. Use the risk factor analysis as a guide: continuous BLE use was identified as a battery drain risk
GitHub
. Perhaps implement a duty cycle for advertising and scanning or use connection parameters that favor low power. On the performance side, monitor memory usage on constrained devices; the code uses a lot of caches and in-memory logs (which is great for performance, but be mindful of memory on say older phones). The multi-tier cache can offload to disk (using memory-mapped files) for L3
GitHub
– ensure that works efficiently on mobile storage. If not already in place, adding a way to persist game state or resume games after an app restart would improve user experience (perhaps checkpoint consensus state to flash periodically). This persistence ties into performance insofar as it must be done efficiently without blocking gameplay.
User Experience & Interface: The current interface is a terminal/TUI suitable for development, but a real-world casino game needs a more user-friendly UI. Designing a graphical UI (with actual dice graphics, betting layouts, etc.) should be a priority for broader adoption. This can be done either by creating a separate frontend that communicates with the Rust backend (for example, a mobile app UI or even a web UI that connects via WebAssembly or a local server). Full feature parity means the UI should expose all features the CLI has: game creation, joining, placing bets, viewing the network status (peers, signal strength perhaps), and seeing token balances. A polished UI will also handle error cases gracefully (e.g., “peer disconnected” messages, etc.). Continued development should allocate time for UI/UX design, especially as we move to Android/iOS (discussed in detail below). Even if we prefer to keep as much logic in Rust as possible, likely the native mobile UI layers or a Rust-based UI framework will need to be implemented. This is an area where collaboration with designers or using platform-native components will pay off in user satisfaction.
Scalability and Interoperability: Beyond the current mesh scope, consider how the system might scale or interoperate with other networks. For example, could BitCraps games on separate meshes (in different locations) be merged if an internet connection becomes available? While not a priority for MVP, the protocol could be extended with an internet gateway or bridge node type that forwards messages from BLE to the internet securely. This would allow remote play or larger multi-hop networks bridging over IP. Interoperability also means ensuring Android and iOS clients (and possibly a desktop client) all adhere to the same protocol and data formats, so they can join the same games seamlessly. The code already defines packet serialization via serde/bincode or similar and is consistent across platforms, which is good
GitHub
GitHub
. Continued development should maintain strict protocol compatibility as changes are made – possibly document the wire format and version it, so older clients can be detected or supported.
Persistent Storage and Analytics: In a casino scenario, it might be useful to store game history or track metrics (for example, to detect if dice are consistently lucky/unlucky beyond randomness, which could indicate a problem). The Efficiency Report mentions a module for memory-efficient game history using delta compression
GitHub
. The team could extend this to disk persistence, so a user’s device keeps a log of past games or earnings. Additionally, implementing a metrics collection module (somewhat alluded to with Prometheus-compatible metrics
GitHub
) would help in production monitoring. For instance, logging metrics like average consensus latency, cache hit rates (already tracked
GitHub
), BLE reconnect frequency, etc., can be invaluable for debugging in the field. Continued development can package these metrics behind a feature flag or optional module, so that during beta testing with real users, engineers can gather performance data and iterate quickly.
Extensibility for New Games or Features: The BitCraps framework – P2P mesh, crypto, consensus, token – could be generalized to support more than just craps. A forward-looking development path is to abstract the gaming logic so that additional games (blackjack, poker, etc.) can plug into the same network and token economy. Already, the protocol is conceptually a generic state machine consensus, with Craps as one implementation. If the codebase is to be expanded, one could introduce an interface or trait for “GameLogic” that defines actions and state transitions, allowing multiple game modules. For now, focus is on craps, but it’s good to keep this in mind to avoid hard-coding things that prevent extension. One simple step might be changing naming (the repo is named bitchat-rust still, which is odd for BitCraps – renaming to BitCraps or a generic name might reduce confusion). Interoperability between different game types on the same mesh could be complex, but not impossible (it might be as simple as namespacing messages by game/table ID which is already done via GameId in the protocol). In sum, as a distributed systems platform, BitCraps can evolve into a broader decentralized gaming framework, and design decisions now should keep that door open.
Module Refactoring and Documentation: Continue the refactoring effort for any oversized modules and maintain up-to-date documentation. A Production Readiness Guide in the docs folder (which I see hints of, e.g. PRODUCTION_READINESS_PLAN.md) is great for new developers or auditors coming to the project. Document any deviations from standard protocols (Noise handshake patterns used, any custom crypto) so that others can review security easily. Encourage the team to write a user guide too – from a senior engineering perspective, having proper end-user documentation will surface any usability issues and drive home where the UI/UX needs improvement. Since the team already has weekly documentation in place
GitHub
, channeling some of that into external docs (README or wiki) will help as the project grows.
Now, with a solid core in Rust, the next big step is deploying BitCraps on Android and iOS devices. Below, I outline how to achieve full feature parity on mobile while sticking as closely as possible to an “all-Rust” development approach.
📱 Expanding to Android and iOS (Rust-Centric Approach)
Porting BitCraps to mobile requires packaging the Rust code into Android and iOS apps with minimal changes. Fortunately, Rust is well-suited for cross-platform development: we can compile the core library for ARM (mobile CPUs) and reuse all of the networking, cryptography, game logic, and consensus code as-is. The goal is to maintain a single Rust codebase for the core, achieving feature parity and interoperability between desktop and mobile. Both Android and iOS can call into Rust libraries, though the integration process differs:
General Strategy: Rust Core as a Shared Library
We will factor the BitCraps code into a library crate that exposes a C-compatible interface (using extern "C" functions or higher-level binding generators). This library will include all core functionality: e.g. functions to initialize the node, discover peers, join/create a game, place bets, roll dice, etc., as well as callbacks for events (like “dice rolled” or “peer connected”). The mobile platform code will then:
Initialize the Rust runtime (if needed) and call an entry-point to start the BitCraps node, providing any platform-specific handles (for example, on Android we might pass in a JNI environment or context if needed for Bluetooth – though btleplug abstracts that).
Use Rust functions for game logic – e.g., when a user taps “Place Bet”, the Swift/Kotlin UI code calls a Rust function place_bet(amount, bet_type) which returns success or failure. The Rust code then propagates that bet to the mesh just as it would on desktop.
Handle callbacks/events – The Rust library can invoke function pointers or use a messaging system to notify the UI of changes (for instance, a closure for “onDiceRolled(dice1, dice2)” could be set during init). This can be done through C callbacks or by polling events via a Rust-provided function.
By keeping logic in Rust, we ensure that an Android phone and an iPhone (and a PC) all speak the exact same protocol and rules – hence they can interoperate in a game session with no special-casing. Below, we discuss each platform in detail.
Android Integration (Rust in an Android app)
Android allows native libraries (in C/C++ or Rust via NDK) to be included in an APK. We will leverage this to include the BitCraps core as a .so library:
Rust Cross-Compilation: Add the Android targets to Rust. For example, to support most devices:
rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android
We can use cargo-ndk to build the Rust library for these targets easily. This produces libbitcraps.so for each ABI.
JNI Binding (Java/Kotlin to Rust): We need a way for the Android app (written in Kotlin/Java) to call our Rust functions. There are two approaches:
Direct JNI in Rust: Use Rust’s jni crate to write #[no_mangle] functions that JNI can call, or use tools like flutter_rust_bridge or uniffi (more on these later) to generate bindings.
C++/NDK glue: Alternatively, create a thin C/C++ layer that interfaces with Rust (since Rust can expose a C API easily) and then call that from Java via JNI. The Mux blog example found it straightforward to write a small C++ wrapper for JNI and call into Rust
mux.com
mux.com
. Either way, some boilerplate is needed because Android doesn’t (yet) have an official Rust->Java bridge.
Using the NDK’s Native Activity is another route: Rust can actually create a native Android Activity entirely in Rust (with a C API that Android calls). However, given we need to integrate with Bluetooth, it’s simpler to use the normal Java Bluetooth stack via btleplug’s Java helper.
Including Bluetooth (btleplug) Support: The btleplug crate supports Android, but as noted in its documentation, Android builds are hybrid Rust/Java and require a bit of setup
docs.rs
. Specifically:
We need to include btleplug’s Java part (droidplug module). The maintainers provide a Gradle build for this. We can either compile it and include the resulting .aar in our app, or include the source as a module in the Android project. The steps are:
a. Include the JNI utility classes (from deviceplug/jni-utils-rs) in the project or via Maven
docs.rs
.
b. Build or reference the btleplug Java code (which basically wraps Android’s BluetoothManager and posts events back to Rust).
c. Use cargo-ndk to build Rust code; ensure the produced lib\*.so files are placed in app/src/main/jniLibs/ for each ABI.
Additional config: Disable certain optimizations like ProGuard from stripping out the unused native code. The btleplug README provides ProGuard rules to keep its classes
docs.rs
.
Permissions: The Android app must request Bluetooth permissions. For Android 12+, this means BLUETOOTH_SCAN, BLUETOOTH_CONNECT (and possibly ACCESS_FINE_LOCATION for scanning). These go in the app’s AndroidManifest.xml and at runtime the app should request them from the user.
Once set up, btleplug on Android “just works” in Rust – our Rust code calls Manager::new() and it invokes the Java underpinnings to scan/advertise. The end result is that an Android device running the app will behave identically to the desktop node in terms of networking and protocol. (Note: because Android’s BLE API differs from iOS, we must test things like whether Android can advertise and scan concurrently. Typically Android can do both in one app process. We should ensure our code handles being both a BLE peripheral and central, which it appears to by managing separate roles in BluetoothTransport if needed.)
UI Layer in Android: We strongly prefer native UI for a quality experience, so we’d implement an Android UI (likely in Kotlin using Jetpack Compose or XML layouts) for the presentation layer. This UI would:
Display the craps table, bets, dice, etc.
Interact with the Rust core via JNI calls. For example, when the user presses “Start Game”, call rust_start_game(), which in Rust initiates discovery or game creation. When a dice roll happens in the Rust consensus, we get a callback and then update the UI (perhaps via a main-thread handler).
Handle Android lifecycle (pause, resume) – we might need to notify the Rust layer to pause scanning if the app goes background, etc., to save battery.
In summary, on Android we will produce a single .apk containing our Rust library and the necessary Java components for Bluetooth. The Kotlin/Java code will be as thin as possible, mainly for UI and glue. This achieves the goal of keeping core logic in Rust. The complexity of the Android build is manageable – it’s been done in similar projects (there’s even a template repository for using btleplug with Flutter, which demonstrates building for Android
docs.rs
). Our approach sticks to native Android, which the team prefers for performance. (Supporting evidence: btleplug’s documentation confirms Android support via JNI and details the build steps
docs.rs
docs.rs
.)
iOS Integration (Rust in an iOS app)
On iOS, we can leverage Rust by creating a static library or framework to embed in an Xcode project. Apple’s tooling is actually friendly to mixing Rust (as C) into Swift/Objective-C apps. Here’s the plan:
Rust Cross-Compilation: Add iOS targets:
rustup target add aarch64-apple-ios x86_64-apple-ios
We will build a universal static library (containing code for both real devices (arm64) and simulator (x86_64) if needed). The cargo-lipo tool greatly simplifies this – it can produce a universal .a or an Xcode framework from our Rust crate. For instance:
cargo lipo --release -p bitcraps_core
would output libbitcraps_core.a containing arm64 + x86_64. Alternatively, with modern Xcode, we might produce an XCFramework.
FFI Interface for Swift: We will expose C ABI functions from Rust that can be called from Swift. Using cbindgen to auto-generate a header file is recommended
docs.rs
. For example, we might have in Rust: #[no_mangle]
pub extern "C" fn start_node() -> u64 { … } #[no_mangle]
pub extern "C" fn place_bet(amount: u32, bet_type: u8) -> bool { … }
// etc.
Then cbindgen produces a bitcraps.h declaring these. We include bitcraps.h in the Xcode project and link with the libbitcraps_core.a. Swift can then call these functions directly via bridging header, or we can write a lightweight Swift wrapper class to make it Swifty.
Bluetooth on iOS: As per btleplug docs, CoreBluetooth on iOS is supported and “should just work” since it shares the implementation with macOS
docs.rs
. We need to do a few things:
In Xcode, enable the Bluetooth capability and add NSBluetoothAlwaysUsageDescription (and/or NSBluetoothPeripheralUsageDescription if needed) to Info.plist
docs.rs
. This ensures the app can use BLE.
When the Rust code calls btleplug::platform::Manager::new(), it will utilize Apple’s CoreBluetooth behind the scenes. We have to make sure a run loop is available. Typically, CoreBluetooth calls must be made from the main thread in iOS. We might need to initialize our Rust networking on the main thread (or otherwise configure btleplug to run its callbacks on the main thread). This is a nuance to test – possibly wrapping the Rust call in a GCD dispatch to main.
iOS also might require the app to actively be scanning/advertising only when in foreground (unless you have certain background modes). Initially, we can assume the game is played with the app open (foreground). If background support is needed (to maintain connections when app is minimized), we’d have to enable the “Uses Bluetooth LE Accessory” background mode and handle the session accordingly.
No additional Obj-C code is strictly required because btleplug uses Apple’s APIs internally (likely through CoreFoundation bindings). If we encounter any issues, an alternative is to implement a small portion in Swift using CoreBluetooth and pass data to Rust, but that defeats the “all in Rust” preference. Given btleplug’s maturity, we anticipate being able to keep it in Rust.
iOS App UI: Similar to Android, we’ll implement a native UI in Swift or SwiftUI for the front-end. The SwiftUI views/controllers will call the Rust functions for actions and perhaps use Combine or callbacks for events coming from Rust. For example, we could expose a Rust function that polls an event queue for “UI events” or use callbacks. Another pattern is to have the Rust core open a small local socket or use notifications, but a simpler way is to use a global function pointer that the Rust code calls on certain events. We can set that from Swift. This is low-level, but effective. Alternatively, since the consensus and game likely run on a background thread (Rust async), we could periodically update UI state via a timer or on-demand. The exact approach can be decided during integration, but the main point is the iOS UI will be responsive and smooth, while Rust does heavy compute in the background (which is ideal, since Rust is native code).
In summary, the iOS integration is straightforward: build Rust as a static library, generate a C header, and call from Swift. This approach has been used in many projects (Mozilla’s Firefox tooling, for instance, uses Rust in iOS via this method). The key is to adhere to Apple’s requirements (proper usage descriptions and main-thread CoreBluetooth usage). After integration, an iPhone and an Android phone should be able to find each other over BLE and play BitCraps together, since the underlying BLE service UUID and protocol are identical. We should verify that the BLE service UUID is allowed on iOS (Apple can sometimes block certain UUIDs or require at least one of the devices to be in peripheral mode – but since our service UUID is custom and we handle roles dynamically, it should be fine). (Supporting evidence: btleplug’s guide outlines iOS integration steps: build a static Rust library, use cbindgen for a header, and add to Xcode
docs.rs
. This matches our plan exactly.)
Maintaining an All-Rust Codebase vs. Using Frameworks
The above plans minimize platform-specific code: the Android UI in Kotlin and iOS UI in Swift are inevitable to provide a good user interface, but all core logic remains in Rust. The question mentions using frameworks but preferring native development. We interpret that as a willingness to use cross-platform UI frameworks if needed, but ideally using the platform’s native tools for performance and polish. A couple of considerations:
Rust-Based Cross-Platform UI: There are emerging GUI frameworks in Rust that target mobile (e.g. Slint or Dioxus). Slint is a declarative UI that can run on iOS/Android with a Rust backend, but it might require a commercial license for closed-source projects and still might not feel 100% native. Dioxus can render a UI using a WebView on mobile (or experimentally with Skia/WGPU)
dioxuslabs.com
. While these allow you to write UI in Rust (which is attractive to keep “entirely in Rust”), they introduce new layers of complexity (webview performance, large bundle size, possibly less native look and feel). Given BitCraps is a relatively graphical app (dice rolling animations, etc.), using the native frameworks (Jetpack Compose and SwiftUI) likely yields a better result and still allows almost all logic in Rust. We recommend the native UI approach with Rust core, as described, to meet the “full feature parity” goal with high fidelity.
Frameworks like Flutter/React Native: These could let us write one UI for both platforms (Flutter in Dart, RN in TS/JS) and call Rust via FFI. This is an option if the team wants to reduce duplicate UI effort. For instance, a Flutter plugin (flutter_rust_bridge) can auto-generate Dart bindings to Rust, and indeed someone created a flutter_btleplug template for BLE in Rust
docs.rs
. However, this means coding in Dart (or JavaScript for RN) for the UI layer, which diverges from the “entirely Rust” philosophy. It also adds a non-trivial runtime (Flutter engine) which might be overkill just for a craps game. Since the prompt emphasizes native and Rust, we lean against this. That said, if time-to-market was critical and the team had Flutter expertise, it’s good to know it’s viable – the Rust core would remain the same and we’d wrap it for Flutter.
Unified API via UniFFI or Bindings Generator: To ease maintaining two platform bindings, we could use a tool like Mozilla’s UniFFI. With UniFFI, you define an interface (in a DSL or attributes on Rust code) and it generates the Kotlin and Swift (and even Python/JS if needed) bindings automatically. This can be incredibly useful to avoid writing JNI and C headers by hand. For example, we could define a Rust function fn create_game(name: String) -> GameHandle in the interface, and UniFFI will generate a Swift class and Kotlin class with a method createGame() that calls into Rust. It handles data conversion, memory, etc. This is a very attractive approach to minimize glue code and potential errors across the two platforms. The only downside is it adds a tool in the toolchain and one must ensure all necessary types are supported (UniFFI supports many common types, and we can wrap our complex ones into supported ones or expose as opaque handles). Given our scenario, this seems worthwhile – it aligns with coding primarily in Rust and saves duplicated effort in writing platform-specific FFI.
In summary, our mobile expansion plan is to reuse 100% of the Rust codebase for the core logic and networking, and write slim native UIs that interface with it. This achieves full feature parity: an Android or iOS device will perform peer discovery, encryption, consensus, and token handling exactly like the current system. Interoperability is ensured by using the same service UUID and protocol – an iPhone peer is indistinguishable from a Linux peer to the Rust code. The few platform differences (permissions, API entry points) are handled in the small amount of platform code. By continuing to develop in Rust, we retain memory safety and performance across all platforms, and we minimize divergence in logic (no need to implement game rules twice). Finally, it’s worth noting that BLE platform nuances will need careful testing. For example, iOS might have stricter timing for advertising packets, Android might handle connection intervals differently – so after implementing the above, plan on a round of testing with various devices (e.g. different Android OEMs, iOS versions) to iron out any cross-platform quirks. The risk of BLE differences was known
GitHub
, and our strategy largely mitigates it by using a cross-platform library. But user-level testing remains key – e.g., ensure the dice roll animation is smooth on a mid-range Android, and that the app doesn’t get killed by the OS for excessive BLE use in background, etc. With this approach, BitCraps can expand beyond the terminal and truly demonstrate its value on ubiquitous mobile devices, delivering a seamless decentralized casino experience whether players are on Android, iOS, or any other Rust-supported platform. The heavy lifting done in the codebase so far – a strong mesh networking core with secure and fair consensus – will serve as a solid foundation as we move into this next phase of development. Sources: The analysis referenced the BitCraps repository code and documentation for specifics on improvements and features
GitHub
GitHub
, as well as official guidance on integrating Rust with mobile platforms (e.g. btleplug project notes on Android/iOS support
docs.rs
docs.rs
). These sources reinforce the feasibility and rationale behind the recommendations made.
