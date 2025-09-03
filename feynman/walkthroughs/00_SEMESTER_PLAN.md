# BitCraps Feynman Semester Plan

This is the canonical curriculum order for a 14‑week semester that teaches Rust, computer architecture, networking, cryptography, math, and distributed systems through the BitCraps codebase.

Each week links to concrete walkthroughs and code. Use this plan rather than relying on raw chapter numbers alone.

## Week 1 — Orientation + Error Handling + Architecture
- Chapter 01: Error Module — `feynman/walkthroughs/01_error_module_walkthrough.md`
- Chapter 03: Library Architecture — `feynman/walkthroughs/03_library_architecture_walkthrough.md`
- Foundation: Computer Architecture for Rust — `feynman/walkthroughs/00A_ARCHITECTURE_MEMORY_MODEL_WALKTHROUGH.md`

## Week 2 — Configuration, Main Application, State
- Chapter 02: Config Module — `feynman/walkthroughs/02_config_module_walkthrough.md`
- Chapter 04: Main Application — `feynman/walkthroughs/04_main_application_walkthrough.md`
- Chapter 07: App State — `feynman/walkthroughs/07_app_state_walkthrough.md`

## Week 3 — Cryptography Fundamentals in Code
- Chapter 05: Crypto Module — `feynman/walkthroughs/05_crypto_module_walkthrough.md`
- Chapter 08: Crypto Encryption — `feynman/walkthroughs/08_crypto_encryption_walkthrough.md`
- Chapter 12: Crypto Safe Arithmetic — `feynman/walkthroughs/12_crypto_safe_arithmetic_walkthrough.md`
- Chapter 13: Crypto Random — `feynman/walkthroughs/13_crypto_random_walkthrough.md`
- Foundation: Crypto Math (ECC/fields) — `feynman/walkthroughs/00B_CRYPTO_MATH_FOUNDATIONS_WALKTHROUGH.md`

## Week 4 — Protocol & Message Formats
- Chapter 16: Protocol Module — `feynman/walkthroughs/16_protocol_module_walkthrough.md`
- Chapter 98: Binary Protocol — `feynman/walkthroughs/98_binary_protocol_walkthrough.md`
- Chapter 170: Transport Layer Architecture — `feynman/walkthroughs/170_transport_layer_architecture_walkthrough.md`

## Week 5 — Transport & NAT
- Chapter 34: Transport Module — `feynman/walkthroughs/34_transport_module_walkthrough.md`
- Chapter 35: Bluetooth Transport — `feynman/walkthroughs/35_bluetooth_transport_walkthrough.md`
- Chapter 36: Enhanced Bluetooth — `feynman/walkthroughs/36_enhanced_bluetooth_walkthrough.md`
- NAT Traversal — `feynman/walkthroughs/211_nat_traversal_walkthrough.md`
- Foundation: Networking Fundamentals (NAT/MTU) — `feynman/walkthroughs/00D_NETWORKING_FUNDAMENTALS_NAT_WALKTHROUGH.md`

## Week 6 — Mesh & Discovery
- Chapter 33: Mesh Networking — `feynman/walkthroughs/33_mesh_networking_walkthrough.md`
- Chapter 38: Kademlia DHT — `feynman/walkthroughs/38_kademlia_dht_walkthrough.md`
- Chapter 18: Network Bridge — `feynman/walkthroughs/18_network_bridge_walkthrough.md`

## Week 7 — Consensus Core
- Chapter 21: Consensus Engine — `feynman/walkthroughs/21_consensus_engine_walkthrough.md`
- Chapter 24: Voting Mechanisms — `feynman/walkthroughs/24_voting_mechanisms_walkthrough.md`
- Chapter 25: Merkle Cache — `feynman/walkthroughs/25_merkle_cache_walkthrough.md`
- Foundation: Distributed Systems Math (quorums/thresholds) — `feynman/walkthroughs/00E_DISTRIBUTED_SYSTEMS_MATH_WALKTHROUGH.md`

## Week 8 — Synchronization & Resilience
- Efficient Sync — `feynman/walkthroughs/20_efficient_sync_walkthrough.md`
- Partition Recovery — `feynman/walkthroughs/190_network_partition_recovery_walkthrough.md`
- Resilience Module — `feynman/walkthroughs/55_resilience_module_walkthrough.md`

## Week 9 — Game Logic & Fairness
- Craps Rules — `feynman/walkthroughs/30_craps_rules_walkthrough.md`
- Gaming Module — `feynman/walkthroughs/29_gaming_module_walkthrough.md`
- Consensus Game Manager — `feynman/walkthroughs/31_consensus_game_manager_walkthrough.md`
- Foundation: Probability & House Edge — `feynman/walkthroughs/00C_PROBABILITY_FAIRNESS_WALKTHROUGH.md`

## Week 10 — Storage & Persistence
- Storage Layer — `feynman/walkthroughs/40_storage_layer_walkthrough.md`
- Database Module — `feynman/walkthroughs/39_database_module_walkthrough.md`
- Persistent Storage — `feynman/walkthroughs/163_persistent_storage_walkthrough.md`

## Week 11 — Security & Sessions
- Security Hardening — `feynman/walkthroughs/69_security_hardening_walkthrough.md`
- Session Management — `feynman/walkthroughs/47_session_management_walkthrough.md`
- Secure Keystore — `feynman/walkthroughs/14_crypto_secure_keystore_walkthrough.md`

## Week 12 — Observability & Performance
- Monitoring Module — `feynman/walkthroughs/53_monitoring_module_walkthrough.md`
- Performance Benchmarks — `feynman/walkthroughs/56_performance_benchmarks_walkthrough.md`
- Caching & Performance — `feynman/walkthroughs/132_caching_performance_walkthrough.md`

## Week 13 — Mobile Integrations
- Android JNI Bridge — `feynman/walkthroughs/60_android_jni_bridge_walkthrough.md`
- iOS Swift FFI — `feynman/walkthroughs/61_ios_swift_ffi_walkthrough.md`
- UniFFI Bindings — `feynman/walkthroughs/62_uniffi_bindings_walkthrough.md`

## Week 14 — Advanced Topics & Launch
- SIMD Crypto Acceleration — `feynman/walkthroughs/70_simd_crypto_acceleration_walkthrough.md`
- Lock‑Free Data Structures — `feynman/walkthroughs/71_lockfree_data_structures_walkthrough.md`
- Treasury Management — `feynman/walkthroughs/141_treasury_management_walkthrough.md`
- Production Deployment — `feynman/walkthroughs/209_production_deployment_strategies_walkthrough.md`

---

Notes
- This plan resolves duplicate topics by selecting a single authoritative chapter per subject.
- Some advanced chapters describe future work; we explicitly label “Implementation Status” in each doc per the updated prompt.
- For the full inventory of all walkthroughs (not in course order), see `feynman/walkthroughs/00_TABLE_OF_CONTENTS.md`.
