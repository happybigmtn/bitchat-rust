# Chapter 0D: Networking Fundamentals — NAT, MTU, and Transport Design
## Deep Dive into NAT traversal, UDP vs BLE constraints, and MTU‑driven fragmentation

---

Implementation Status: Implemented/Partial
- Lines of code analyzed: ~800 (transport/*)
- Key files: `src/transport/nat_traversal.rs`, `src/transport/security.rs`, `src/transport/secure_gatt_server.rs`
- Gaps/Future Work: End‑to‑end MTU autotuning and fragmentation benchmarking

## Fundamentals

- NAT types and strategies: STUN, TURN, hole‑punching; symmetric NAT caveats
- MTU sizing: headers, nonces, AAD; why 244‑byte BLE “budget” matters
- Multipath and retries: when/how to backoff and detect partitions

## Code Mapping

- NAT handler orchestration and capability detection
- Fragment header design (`FRAGMENT_HEADER_SIZE`) and sequence handling
- Hints for adaptive fragmentation and retransmission strategies

## Senior Engineering Review
- Track per‑path MTU and error rates; feed into fragmentation strategy
- Add metrics for retransmits, out‑of‑order, and reassembly failures

## Lab Exercise
- Capture NAT types in your environment and test hole‑punching
- Measure goodput under varying MTUs; chart overhead vs payload size

## Check Yourself
- Why do symmetric NATs need relays?
- How does AEAD overhead affect maximum plaintext per packet?
- When is TCP preferable to UDP/ BLE for this system?
