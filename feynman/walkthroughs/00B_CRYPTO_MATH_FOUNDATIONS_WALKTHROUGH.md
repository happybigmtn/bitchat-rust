# Chapter 0B: Crypto Math Foundations — Complete Implementation Analysis
## Deep Dive into finite fields, elliptic curves, Diffie–Hellman, and AEAD mapping to BitCraps

---

Implementation Status: Implemented (via X25519 + ChaCha20Poly1305)
- Lines of code analyzed: ~200 (crypto/encryption.rs, transport/keystore.rs)
- Key files: `src/crypto/encryption.rs`, `src/transport/keystore.rs`
- Gaps/Future Work: Formal proofs out of scope; add curve ops visualization in docs

## Mathematical Foundations

- Finite field arithmetic (mod p), group law over Montgomery/Edwards curves
- X25519 ECDH: scalar mult k·P yields shared secret; one‑way hardness via discrete log
- HKDF: extract/expand to domain‑separated symmetric keys
- AEAD: ChaCha20Poly1305 provides confidentiality + integrity using a 96‑bit nonce

Worked Example
- Given private scalars a, b, public keys A=a·G, B=b·G, both compute S=a·B=b·A
- HKDF(S || salt) → K_symmetric; encrypt with ChaCha20Poly1305(nonce, aad, plaintext)

## Code Mapping

- `src/crypto/encryption.rs`
  - EphemeralSecret/PublicKey from `x25519_dalek`
  - HKDF over SHA‑256, 32‑byte symmetric key, 12‑byte nonce
  - AEAD encrypt/decrypt; AAD binds metadata to ciphertext

- `src/transport/keystore.rs`
  - Key backup/restore via password‑derived keys
  - Nonce management and constant‑time comparisons

## Senior Engineering Review
- Validate nonce uniqueness per key; rotate keys on schedule (`transport/security.rs`)
- Consider domain separation labels in HKDF context info
- Add misuse‑resistant API wrappers that take `&Nonce` to prevent accidental reuse

## Lab Exercise
- Implement a test that ECDH keys match across peers and decrypt succeeds
- Extend AAD to include a message counter; verify tamper detection

## Check Yourself
- Why is Diffie–Hellman secure against passive eavesdropping?
- What happens if nonces repeat under the same key with ChaCha20Poly1305?
- Why use HKDF after ECDH instead of the raw shared secret?
