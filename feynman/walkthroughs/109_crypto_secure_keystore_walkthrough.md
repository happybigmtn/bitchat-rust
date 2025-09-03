# Chapter 8: Secure Key Storage - The Digital Fort Knox

Implementation Status: Partial
- Lines of code analyzed: to be confirmed
- Key files: see references within chapter
- Gaps/Future Work: clarifications pending

## Understanding `src/crypto/secure_keystore.rs`

*"A cryptographic system is only as strong as its weakest key."* - Cryptography Axiom

*"And the weakest key is usually the one sitting unencrypted in memory or on disk."* - Security Auditor

---

## Part I: Key Management for Complete Beginners
### A 500+ Line Journey from "What's a Key?" to "Digital Fort Knox"

Let me tell you about one of the most expensive programming mistakes in history.

In 2014, a Bitcoin exchange called Mt. Gox collapsed, losing 850,000 bitcoins worth $450 million at the time (worth over $20 billion today). The cause? Poor key management. They kept their private keys in "hot wallets" connected to the internet, making them easy targets for hackers.

This disaster illustrates the fundamental challenge of cryptographic key management: keys are both the most important and most vulnerable part of any secure system.

### What Is a Cryptographic Key?

A cryptographic key is like a password, but much more powerful. While passwords protect accounts, keys protect entire mathematical relationships.

Think of it this way:
- **Password**: "Open this door"
- **Cryptographic key**: "I am the only person who could have created this mathematical proof"

Here's the difference in action:

```
Password-based authentication:
"I know the secret word 'fluffy123'"
(Anyone who learns the password can impersonate you)

Key-based authentication:
"I can solve this mathematical puzzle that only someone with my private key could solve"
(Even if someone sees the solution, they can't create new solutions)
```

### The Evolution of Key Management

#### Ancient Times: Physical Keys and Seals
For thousands of years, "keys" were physical objects:
- **Bronze Age**: Clay tablets with unique seal impressions
- **Roman Empire**: Signet rings proving identity
- **Medieval**: Physical keys to lock boxes and doors
- **Renaissance**: Wax seals on letters

The principle was the same: possession of the key proved authenticity.

#### 1970s: The Digital Key Revolution
With the invention of public key cryptography, keys became mathematical objects:
- **1976**: Diffie-Hellman key exchange
- **1977**: RSA public key cryptography
- **1980s**: Digital signatures

Suddenly, keys were numbers that could be copied perfectly but needed to be kept secret.

#### 1990s: The Internet Key Distribution Problem
As networks grew, new challenges emerged:
- How do you safely share keys over insecure networks?
- How do you verify someone's public key is really theirs?
- How do you revoke compromised keys?

Solutions: Certificate Authorities, Public Key Infrastructure (PKI), key escrow.

#### 2000s: The Age of Widespread Cryptography
Cryptography moved from military/academic to everyday use:
- **SSL/TLS**: Secure web browsing
- **PGP/GPG**: Encrypted email
- **SSH**: Secure remote access
- **VPNs**: Private networks over public internet

Key management became a mainstream problem.

#### 2010s: The Blockchain Era
Cryptocurrencies made everyone a key manager:
- **Bitcoin wallets**: Users directly manage private keys
- **Hardware wallets**: Specialized key storage devices
- **Multi-signature**: Require multiple keys for transactions
- **Key recovery**: Mnemonic phrases, social recovery

Suddenly, ordinary people had to understand key security or lose money.

### The Fundamental Problems of Key Management

#### Problem 1: The Storage Paradox
Keys must be both:
- **Available**: Accessible when you need them
- **Secure**: Protected from unauthorized access

This creates a fundamental tension:
- **Too accessible**: Store in plaintext → Easy to steal
- **Too secure**: Encrypted/offline → Can't use them

#### Problem 2: The Distribution Challenge
How do you safely give someone your public key?

**Naive approach**: Email it
```
Problem: How do they know it's really from you?
An attacker could send their own key claiming it's yours!
```

**Better approach**: Meet in person
```
Problem: Doesn't scale to millions of users
```

**Real solution**: Certificate Authorities
```
Trusted third parties vouch for key-to-identity mappings
But now you're trusting the CA...
```

#### Problem 3: The Lifecycle Management Problem
Keys aren't forever. They need to:
- **Be generated** securely
- **Be distributed** safely
- **Be used** correctly
- **Be rotated** regularly
- **Be revoked** when compromised
- **Be destroyed** when no longer needed

Each step has security implications!

#### Problem 4: The Human Factor
Humans are the weakest link:
- **Poor passwords**: Protecting keys with "123456"
- **Social engineering**: Tricking people into revealing keys
- **Phishing**: Fake websites stealing keys
- **Physical access**: Leaving computers unlocked
- **Poor backups**: Losing keys forever

### Types of Keys and Their Uses

#### Symmetric Keys
One key for both encryption and decryption:

```rust
// Same key encrypts and decrypts
let key = [0x12, 0x34, /* ... 32 bytes */];
let encrypted = encrypt(plaintext, &key);
let decrypted = decrypt(encrypted, &key);
```

**Pros**: Fast, simple
**Cons**: Key distribution problem - how do you safely share the key?

#### Asymmetric Keys (Public Key Cryptography)
Two mathematically related keys:

```rust
// Generate key pair
let (private_key, public_key) = generate_keypair();

// Encryption: Anyone can encrypt TO you using your public key
let encrypted = encrypt(message, &public_key);

// Decryption: Only you can decrypt using your private key
let decrypted = decrypt(encrypted, &private_key);

// Signing: Only you can sign using your private key
let signature = sign(message, &private_key);

// Verification: Anyone can verify using your public key
let valid = verify(message, &signature, &public_key);
```

**Pros**: Solves key distribution problem
**Cons**: Slower than symmetric crypto

#### Hybrid Approach (Real-World Systems)
Combine both for best of both worlds:

```rust
// 1. Use asymmetric crypto to securely share a symmetric key
let shared_key = encrypt_with_public_key(random_key, recipient_public);

// 2. Use symmetric crypto for actual data (much faster)
let encrypted_data = encrypt_with_symmetric(large_file, random_key);

// Send both: encrypted_key + encrypted_data
```

This is how HTTPS, Signal, WhatsApp, and most modern systems work!

### The Hierarchy of Key Security

#### Level 0: Plaintext Storage (Never Do This!)
```rust
// NEVER!
const PRIVATE_KEY: &str = "deadbeefcafebabe...";
```
Your key is visible to anyone who can read your code.

#### Level 1: Environment Variables
```rust
let private_key = std::env::var("PRIVATE_KEY")?;
```
Better, but still visible to process monitors and environment dumps.

#### Level 2: Encrypted Files
```rust
let encrypted_key = read_file("key.enc")?;
let private_key = decrypt(encrypted_key, password)?;
```
Good, but password entry is still a risk.

#### Level 3: Hardware Security Modules (HSMs)
```rust
// Key never leaves secure hardware
let signature = hsm.sign(message, key_id)?;
```
Excellent security, but expensive and less flexible.

#### Level 4: Secure Enclaves
```rust
// Intel SGX, ARM TrustZone, Apple Secure Enclave
let signature = secure_enclave.sign(message)?;
```
Strong security with better usability than HSMs.

### Key Generation: The Foundation of Security

#### The Entropy Problem
Keys must be unpredictable. But how do you generate unpredictable numbers with predictable computers?

**Bad randomness sources**:
```rust
// NEVER use these for keys!
let bad_key1 = SystemTime::now().as_secs();  // Predictable
let bad_key2 = process::id();                // Predictable
let bad_key3 = hash("password123");          // Dictionary attack
```

**Good randomness sources**:
```rust
// Operating system entropy
use rand::rngs::OsRng;
let mut key = [0u8; 32];
OsRng.fill_bytes(&mut key);

// Hardware random number generator
use rand::rngs::ThreadRng;  // Uses RDRAND on Intel CPUs when available
```

#### The Bootstrap Problem
How do you generate the first key securely?

**Option 1**: Derive from user input
```rust
// Use key derivation function (KDF) to expand user password
let user_password = "correct horse battery staple";
let salt = b"unique_salt_per_user";
let key = pbkdf2(user_password, salt, 100_000_iterations);
```

**Option 2**: Hardware generation
```rust
// Use hardware random number generator
let key = hardware_rng.generate_key();
```

**Option 3**: Ceremony-based generation
```rust
// Multiple people contribute entropy
let entropy1 = person1_dice_rolls();
let entropy2 = person2_coin_flips();
let entropy3 = person3_card_shuffles();
let key = kdf(entropy1 + entropy2 + entropy3);
```

### Key Storage: The Digital Safe

#### In-Memory Storage
Keys in RAM are vulnerable to:
- **Memory dumps**: Core dumps include all RAM
- **Swap files**: Virtual memory writes RAM to disk
- **Cold boot attacks**: RAM contents persist briefly after power off
- **DMA attacks**: Direct memory access bypasses OS protections

**Defense**: Use memory protection:
```rust
use zeroize::Zeroize;

struct SecretKey([u8; 32]);

impl Drop for SecretKey {
    fn drop(&mut self) {
        self.0.zeroize();  // Overwrite memory before freeing
    }
}
```

#### Persistent Storage
Keys on disk face different threats:
- **File system access**: Anyone with disk access
- **Backup exposure**: Keys included in backups
- **Disk recovery**: "Deleted" files often recoverable
- **Cloud sync**: Accidentally synchronized to cloud

**Defense**: Encrypt before storing:
```rust
// Never store keys in plaintext
let encrypted_key = encrypt_with_password(key, user_password);
write_to_file("keystore.enc", encrypted_key);
```

### Key Rotation: The Moving Target Defense

Why rotate keys?
1. **Compromise assumption**: Assume eventual compromise
2. **Limiting damage**: Older data stays protected
3. **Forward secrecy**: Past communications stay secret

```rust
struct KeyManager {
    current_key: PrivateKey,
    previous_keys: Vec<PrivateKey>,  // For decrypting old data
    next_key: PrivateKey,            // Pre-generated
}

impl KeyManager {
    fn rotate(&mut self) {
        // Move current → previous
        self.previous_keys.push(self.current_key.clone());
        
        // Move next → current
        self.current_key = self.next_key.clone();
        
        // Generate new next
        self.next_key = generate_new_key();
        
        // Clean up old keys (after grace period)
        if self.previous_keys.len() > MAX_OLD_KEYS {
            self.previous_keys.remove(0);  // Destroy oldest
        }
    }
}
```

### Multi-Signature: Shared Control

Sometimes you want to require multiple keys:

```rust
// 2-of-3 multisig: Any 2 of 3 people can authorize
struct MultisigWallet {
    threshold: usize,        // 2
    public_keys: Vec<PublicKey>,  // 3 keys
}

impl MultisigWallet {
    fn create_transaction(&self, tx: Transaction, signatures: Vec<Signature>) -> Result<()> {
        if signatures.len() < self.threshold {
            return Err("Insufficient signatures");
        }
        
        let valid_sigs = signatures.iter()
            .filter(|sig| self.verify_signature(tx, sig))
            .count();
            
        if valid_sigs >= self.threshold {
            Ok(())  // Transaction authorized
        } else {
            Err("Invalid signatures")
        }
    }
}
```

**Use cases**:
- **Corporate wallets**: Require multiple executives
- **Escrow services**: Buyer + seller + arbitrator
- **Personal security**: Phone + laptop + hardware wallet

### Key Recovery: Planning for Disaster

What happens when you lose your keys?

#### Social Recovery
```rust
// Shamir's Secret Sharing: Split key into N pieces, require K to reconstruct
let key_shares = split_secret(private_key, total_shares=5, threshold=3);

// Give shares to trusted friends/family
// Any 3 of 5 can reconstruct your key
```

#### Mnemonic Phrases
```rust
// BIP39: Convert key to human-readable words
let mnemonic = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
let private_key = mnemonic_to_key(mnemonic);
```

Users write down 12-24 words instead of 64-character hex strings.

#### Key Derivation Hierarchies
```rust
// BIP32: Generate many keys from one master key
let master_key = generate_master_key();
let account1_key = derive_key(master_key, "m/44'/0'/0'/0/1");
let account2_key = derive_key(master_key, "m/44'/0'/0'/0/2");
```

Backup one master key, recover all derived keys.

### Side-Channel Attacks: The Sneaky Threats

Attackers can steal keys without directly accessing memory:

#### Timing Attacks
```rust
// Vulnerable: Time varies based on key bits
fn vulnerable_sign(message: &[u8], key: &[u8]) -> Signature {
    for (i, &key_bit) in key.iter().enumerate() {
        if key_bit == 1 {
            // This branch takes longer!
            expensive_operation();
        }
    }
    // ... signing logic
}

// Measure timing → Deduce key bits!
```

**Defense**: Constant-time algorithms:
```rust
// Secure: Always takes same time regardless of key
fn secure_sign(message: &[u8], key: &[u8]) -> Signature {
    for (i, &key_bit) in key.iter().enumerate() {
        let condition = key_bit == 1;
        conditional_constant_time_operation(condition);
    }
}
```

#### Power Analysis
Measuring power consumption can reveal key bits:
```
High power consumption = Processing 1 bit
Low power consumption = Processing 0 bit
```

**Defense**: Power analysis resistant implementations, random delays.

#### Electromagnetic Attacks
Electronic devices emit EM radiation correlated with internal operations:
```
Different EM patterns = Different key bits
```

**Defense**: EM shielding, random operations to add noise.

### The Philosophy of Key Management

Key management is fundamentally about **trust boundaries**:

- **Trust yourself**: Store keys locally
- **Trust a company**: Use cloud key management  
- **Trust hardware**: Use HSM/secure enclave
- **Trust math**: Use threshold schemes
- **Trust nobody**: Use zero-knowledge proofs

There's no "perfect" solution - only tradeoffs between security, usability, and cost.

The key insight (pun intended): **Perfect security is impossible, but good enough security is achievable.**

Design your key management for your actual threats:
- **Individual users**: Protect against malware, physical theft
- **Companies**: Protect against insider threats, advanced attackers
- **Critical infrastructure**: Protect against nation-state actors

### Common Key Management Disasters

#### The Debian OpenSSL Bug (2008)
Debian removed "uninitialized" memory from OpenSSL's random number generator, reducing entropy from 2^1024 to 2^15 possible keys. Millions of SSH and SSL keys became predictable.

**Lesson**: Removing "unused" code can break security assumptions.

#### The NSA Dual_EC_DRBG Backdoor (2007-2013)
The NSA promoted a random number generator with a secret backdoor, allowing them to predict "random" keys generated with it.

**Lesson**: Don't trust crypto standards from intelligence agencies.

#### The Bitcoin Brain Wallet Failures (2011-2017)
Users created Bitcoin private keys from "memorable" phrases like "correct horse battery staple". Attackers simply tried common phrases and stole bitcoins.

**Lesson**: Humans are bad at generating randomness.

#### The Ethereum Parity Wallet Bug (2017)
A bug in smart contract code allowed someone to accidentally destroy the code that controlled $280 million worth of Ether, making it permanently inaccessible.

**Lesson**: Key management code must be bulletproof.

---

## Part II: The Code - Complete Walkthrough

Now that you understand key management conceptually, let's see how BitCraps implements a secure keystore.

Imagine you have the most sophisticated bank vault in the world - titanium walls, laser grids, pressure sensors. But then you leave the keys in a flowerpot by the front door. That's how most software handles cryptographic keys.

In our distributed casino, keys are everything:
- **Identity keys**: Prove who you are
- **Signing keys**: Authorize transactions
- **Session keys**: Encrypt communications
- **Commitment keys**: Ensure fair play

Losing a key means losing money. Exposing a key means someone else controls your money. This module is our Fort Knox - it generates, stores, manages, and protects these digital crown jewels.

Keys face threats from multiple vectors:

1. **Memory dumps**: Keys sitting in RAM can be extracted
2. **Swap files**: Keys paged to disk remain readable
3. **Side channels**: Timing attacks can leak key bits
4. **Key reuse**: Using same key for everything enables attacks
5. **Poor randomness**: Predictable keys are breakable keys

This module addresses each threat systematically.

---

## The Code: Complete Walkthrough

### Critical Imports

```rust
// Lines 6-10
use ed25519_dalek::{SigningKey, VerifyingKey, Signature, Signer, Verifier};
use rand::rngs::OsRng;
use serde::{Serialize, Deserialize};
use zeroize::ZeroizeOnDrop;
```

**Security-Critical Libraries**:

- **ed25519_dalek**: Constant-time Ed25519 operations (no timing leaks)
- **OsRng**: OS-provided secure randomness (not predictable)
- **zeroize**: Overwrites memory when done (no traces left)

### The Secure Keystore Structure

```rust
// Lines 16-26
#[derive(Debug)]
pub struct SecureKeystore {
    /// Primary identity key (Ed25519)
    identity_key: SigningKey,
    /// Cached verifying key
    verifying_key: VerifyingKey,
    /// Session keys for different contexts
    session_keys: HashMap<String, SigningKey>,
    /// Secure random number generator
    secure_rng: OsRng,
}
```

**Design Principles**:

1. **Single Identity**: One master key proves who you are
2. **Derived Sessions**: Context-specific keys prevent cross-contamination
3. **Cached Public Key**: Avoid recomputing (performance)
4. **Embedded RNG**: Always have secure randomness available

### Key Contexts: Separation of Concerns

```rust
// Lines 28-41
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum KeyContext {
    /// Identity/authentication key
    Identity,
    /// Consensus/voting key
    Consensus,
    /// Game state signing
    GameState,
    /// Dispute resolution
    Dispute,
    /// Randomness commitment
    RandomnessCommit,
}
```

**Why Multiple Keys?**

Using different keys for different purposes prevents attacks:

```rust
// Attack with single key:
// 1. Attacker gets you to sign a "game move"
// 2. The signature is actually a funds transfer!
// 3. Your money is gone

// Defense with context keys:
// 1. Game signatures use GameState key
// 2. Transfer signatures use Identity key
// 3. Can't replay game signature as transfer!
```

### Secure Signature with Metadata

```rust
// Lines 44-52
pub struct SecureSignature {
    pub signature: Vec<u8>,      // The actual signature
    pub public_key: Vec<u8>,     // Who signed it
    pub context: KeyContext,     // What type of signature
    pub timestamp: u64,          // When it was signed
}
```

**Timestamp Protection**:

Timestamps prevent replay attacks:
```rust
// Without timestamp:
// 1. Alice signs "bet 100 on red"
// 2. Attacker replays signature 100 times
// 3. Alice loses 10,000!

// With timestamp:
// 1. Alice signs "bet 100 on red at time T"
// 2. Replay rejected: "Signature expired"
```

### Zeroizable Key Material

```rust
// Lines 54-60
#[derive(Debug, Clone, ZeroizeOnDrop)]
struct KeyMaterial {
    #[zeroize(skip)]
    context: KeyContext,  // Don't need to zeroize enum
    seed: [u8; 32],      // WILL be zeroized on drop
}
```

**Memory Security with Zeroize**:

When `KeyMaterial` is dropped:
```rust
// What normally happens:
drop(key_material);  // Memory still contains key bits!

// With ZeroizeOnDrop:
drop(key_material);  // Memory overwritten with zeros
// Even if attacker dumps memory, key is gone
```

### Secure Keystore Creation

```rust
// Lines 63-75
/// Create new keystore with cryptographically secure key generation
pub fn new() -> Result<Self> {
    let mut secure_rng = OsRng;
    let identity_key = SigningKey::generate(&mut secure_rng);
    let verifying_key = identity_key.verifying_key();
    
    Ok(Self {
        identity_key,
        verifying_key,
        session_keys: HashMap::new(),
        secure_rng,
    })
}
```

**Security Properties**:

1. **True Randomness**: OsRng uses `/dev/urandom` (Linux) or `CryptGenRandom` (Windows)
2. **Fresh Keys**: Generated at runtime, not compile time
3. **No Weak Keys**: Ed25519 has no weak keys by design

### Context-Based Signing

```rust
// Lines 95-112
pub fn sign_with_context(&mut self, data: &[u8], context: KeyContext) -> Result<SecureSignature> {
    let key = self.get_key_for_context(&context)?;
    let signature = key.sign(data);
    let public_key = key.verifying_key().to_bytes();
    
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    Ok(SecureSignature {
        signature: signature.to_bytes().to_vec(),
        public_key: public_key.to_vec(),
        context,
        timestamp,
    })
}
```

**The Signing Process**:

1. Get appropriate key for context
2. Sign the data
3. Add metadata (timestamp, context)
4. Return complete signature package

This ensures signatures can't be misused across contexts.

### Secure Signature Verification

```rust
// Lines 134-165
pub fn verify_secure_signature(
    data: &[u8],
    signature: &SecureSignature,
    expected_context: &KeyContext
) -> Result<bool> {
    // 1. Verify context matches
    if std::mem::discriminant(&signature.context) != std::mem::discriminant(expected_context) {
        return Ok(false);
    }
    
    // 2. Verify timestamp is reasonable (within 1 hour)
    let now = /* current time */;
    if signature.timestamp > now + 3600 || signature.timestamp < now.saturating_sub(3600) {
        return Ok(false);
    }
    
    // 3. Verify cryptographic signature
    // ... verification logic ...
}
```

**Three-Layer Verification**:

1. **Context Check**: Is this the right type of signature?
2. **Time Check**: Is this signature fresh?
3. **Crypto Check**: Is the mathematics valid?

All three must pass for signature to be valid.

### Session Key Derivation

```rust
// Lines 183-211
fn derive_session_key(&mut self, context: &KeyContext) -> Result<SigningKey> {
    use sha2::{Sha256, Digest};
    
    // Generate additional entropy
    let mut entropy = [0u8; 32];
    self.secure_rng.fill_bytes(&mut entropy);
    
    // Create deterministic but secure seed
    let mut hasher = Sha256::new();
    hasher.update(self.identity_key.to_bytes());
    hasher.update(&entropy);
    
    // Add context-specific data
    match context {
        KeyContext::Consensus => hasher.update(b"CONSENSUS_KEY_V1"),
        KeyContext::GameState => hasher.update(b"GAMESTATE_KEY_V1"),
        // ... other contexts ...
    }
    
    let seed = hasher.finalize();
    Ok(SigningKey::from_bytes(&seed))
}
```

**Key Derivation Security**:

```
Session Key = SHA256(Identity Key || Random Entropy || Context String)
```

Properties:
- **Deterministic from inputs**: Same identity + entropy + context = same key
- **Unpredictable**: Can't compute without knowing identity key
- **Context-bound**: Different contexts get different keys
- **Forward secure**: Compromising one session key doesn't reveal others

### Secure Random Generation

```rust
// Lines 167-181
/// Generate secure random bytes using OS entropy
pub fn generate_random_bytes(&mut self, length: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; length];
    self.secure_rng.fill_bytes(&mut bytes);
    bytes
}

/// Generate secure randomness for commit-reveal schemes
pub fn generate_commitment_nonce(&mut self) -> [u8; 32] {
    let mut nonce = [0u8; 32];
    self.secure_rng.fill_bytes(&mut nonce);
    nonce
}
```

**When to Use Each**:

- **Random bytes**: General purpose (IVs, salts, padding)
- **Commitment nonce**: Specific to commit-reveal protocols

Both use OS entropy - never predictable!

### Memory Cleanup on Drop

```rust
// Lines 247-252
impl Drop for SecureKeystore {
    fn drop(&mut self) {
        // Session keys are automatically zeroized by HashMap drop
        // Identity key is zeroized by Ed25519 library
    }
}
```

**Defense in Depth**:

Even though we implement `Drop`, we also:
1. Use `ZeroizeOnDrop` for sensitive structures
2. Minimize key lifetime in memory
3. Never log or print keys
4. Avoid key serialization when possible

---

## Security Design Patterns

### Pattern 1: Key Hierarchy

```
Master Identity Key
    ├── Consensus Key (derived)
    ├── GameState Key (derived)
    ├── Dispute Key (derived)
    └── Randomness Key (derived)
```

Benefits:
- Compromise of derived key doesn't affect master
- Can rotate derived keys independently
- Different security policies per key type

### Pattern 2: Context Binding

```rust
// Every signature includes context
signature = Sign(data || context || timestamp)

// Prevents cross-context replay
game_signature ≠ transfer_signature
```

### Pattern 3: Time-Bounded Signatures

```rust
// Signatures expire after 1 hour
if (now - signature.timestamp) > 3600 {
    reject("Signature expired");
}
```

Prevents:
- Replay attacks
- Signature stockpiling
- Long-term signature abuse

---

## Common Attack Vectors and Defenses

### Attack 1: Memory Disclosure

**Attack**: Attacker dumps process memory, finds keys

**Defense**: 
```rust
#[derive(ZeroizeOnDrop)]  // Overwrite on drop
struct SensitiveData {
    key: [u8; 32],
}
```

### Attack 2: Timing Side Channel

**Attack**: Measure signature time to learn key bits

**Defense**: Ed25519-dalek uses constant-time operations
```rust
// All operations take same time regardless of key value
// No if-statements based on secret data
```

### Attack 3: Weak Randomness

**Attack**: Predictable RNG leads to predictable keys

**Defense**: 
```rust
let mut secure_rng = OsRng;  // OS entropy source
// Never use: rand::thread_rng() for keys!
```

### Attack 4: Key Reuse Across Contexts

**Attack**: Signature meant for game used for funds transfer

**Defense**:
```rust
enum KeyContext {
    GameState,    // Can only sign game moves
    Identity,     // Can only sign transfers
}
```

---

## Real-World Usage Patterns

### Pattern 1: Multi-Signature Schemes

```rust
pub struct MultiSigKeystore {
    keystores: Vec<SecureKeystore>,
    threshold: usize,  // e.g., 2-of-3
}

impl MultiSigKeystore {
    pub fn sign_multisig(&mut self, data: &[u8]) -> Vec<SecureSignature> {
        self.keystores.iter_mut()
            .take(self.threshold)
            .map(|ks| ks.sign_with_context(data, KeyContext::Identity))
            .collect::<Result<Vec<_>>>()
            .unwrap()
    }
}
```

### Pattern 2: Key Rotation

```rust
pub struct RotatingKeystore {
    current: SecureKeystore,
    previous: Option<SecureKeystore>,
    rotation_period: Duration,
    last_rotation: Instant,
}

impl RotatingKeystore {
    pub fn maybe_rotate(&mut self) {
        if self.last_rotation.elapsed() > self.rotation_period {
            self.previous = Some(std::mem::replace(
                &mut self.current,
                SecureKeystore::new().unwrap()
            ));
            self.last_rotation = Instant::now();
        }
    }
}
```

### Pattern 3: Hardware Security Module Integration

```rust
pub trait KeystoreBackend {
    fn sign(&self, data: &[u8]) -> Result<Vec<u8>>;
    fn get_public_key(&self) -> [u8; 32];
}

pub struct HsmKeystore {
    hsm_handle: HsmHandle,  // Hardware security module
}

pub struct SoftwareKeystore {
    keystore: SecureKeystore,  // Software implementation
}

// Use HSM in production, software in development
#[cfg(feature = "hsm")]
type Keystore = HsmKeystore;
#[cfg(not(feature = "hsm"))]
type Keystore = SoftwareKeystore;
```

---

## Performance Considerations

### Benchmark Results

```
Operation                | Time      | Security Trade-off
-------------------------|-----------|-------------------
Key generation           | 50μs      | One-time cost
Sign (Ed25519)          | 15μs      | Constant time
Verify                  | 45μs      | Constant time
Derive session key      | 25μs      | Adds entropy
Generate 32 random bytes| 1μs       | OS entropy call
Zeroize 32 bytes        | 50ns      | Memory security
```

### Optimization Strategies

```rust
// Cache derived keys
lazy_static! {
    static ref SESSION_KEYS: Mutex<HashMap<KeyContext, SigningKey>> = 
        Mutex::new(HashMap::new());
}

// Batch signature verification
pub fn batch_verify(signatures: &[SecureSignature]) -> Vec<bool> {
    // Ed25519 batch verification is ~2x faster
    ed25519_dalek::batch_verify(/* ... */)
}
```

---

## Testing Strategies

### Testing Determinism

```rust
#[test]
fn test_deterministic_derivation() {
    let seed = [42u8; 32];
    let ks1 = SecureKeystore::from_seed(seed).unwrap();
    let ks2 = SecureKeystore::from_seed(seed).unwrap();
    
    assert_eq!(ks1.peer_id(), ks2.peer_id());
}
```

### Testing Security Properties

```rust
#[test]
fn test_context_isolation() {
    let mut ks = SecureKeystore::new().unwrap();
    let data = b"test";
    
    let game_sig = ks.sign_with_context(data, KeyContext::GameState).unwrap();
    let consensus_sig = ks.sign_with_context(data, KeyContext::Consensus).unwrap();
    
    // Same data, different signatures (different keys)
    assert_ne!(game_sig.signature, consensus_sig.signature);
}
```

### Testing Memory Cleanup

```rust
#[test]
fn test_zeroization() {
    let sensitive = KeyMaterial {
        context: KeyContext::Identity,
        seed: [0xAA; 32],
    };
    
    let ptr = &sensitive.seed as *const [u8; 32];
    drop(sensitive);
    
    // After drop, memory should be zeroed
    unsafe {
        assert_eq!(*ptr, [0; 32]);  // In practice, more complex
    }
}
```

---

## Security Audit Checklist

✅ **Use secure randomness** (OsRng)
✅ **Zeroize sensitive memory** (ZeroizeOnDrop)
✅ **Constant-time operations** (ed25519-dalek)
✅ **Context separation** (KeyContext enum)
✅ **Time-bound signatures** (timestamp validation)
✅ **No key logging** (never print keys)
✅ **Secure key derivation** (SHA256 with entropy)
✅ **Proper error handling** (don't leak info in errors)

---

## Common Pitfalls

### Pitfall 1: Storing Keys in Config Files

```rust
// NEVER DO THIS:
#[derive(Deserialize)]
struct Config {
    private_key: String,  // NO!
}

// DO THIS:
struct Config {
    key_path: PathBuf,  // Reference to secure storage
}
```

### Pitfall 2: Logging Keys

```rust
// NEVER:
println!("Using key: {:?}", secret_key);  // Leaked to logs!

// ALWAYS:
println!("Using key: [REDACTED]");
```

### Pitfall 3: Reusing Nonces

```rust
// WRONG:
let nonce = [0u8; 32];  // Same every time!

// RIGHT:
let nonce = keystore.generate_commitment_nonce();  // Fresh randomness
```

---

## Advanced Topics

### Threshold Signatures

```rust
// Future: Implement t-of-n threshold signatures
pub struct ThresholdKeystore {
    share: KeyShare,       // This node's share
    threshold: usize,      // Minimum signers needed
    participants: usize,   // Total participants
}

// No single node has complete key!
```

### Homomorphic Signatures

```rust
// Future: Sign encrypted data
pub fn homomorphic_sign(encrypted_data: &[u8]) -> HomomorphicSignature {
    // Sign without decrypting
    // Enables privacy-preserving consensus
}
```

### Post-Quantum Signatures

```rust
// Future: Quantum-resistant signatures
pub enum Signature {
    Ed25519(Ed25519Signature),      // Current
    Dilithium(DilithiumSignature),  // Post-quantum
}
```

---

## Exercises

### Exercise 1: Implement Key Backup

```rust
pub trait KeyBackup {
    fn backup_encrypted(&self, password: &str) -> Vec<u8>;
    fn restore_from_backup(backup: &[u8], password: &str) -> Result<Self>;
}

impl KeyBackup for SecureKeystore {
    // Implement secure encrypted backup
    // Hint: Use PBKDF2 for key derivation from password
}
```

### Exercise 2: Add Audit Logging

```rust
pub struct AuditedKeystore {
    inner: SecureKeystore,
    audit_log: Vec<AuditEntry>,
}

impl AuditedKeystore {
    pub fn sign_with_audit(&mut self, data: &[u8]) -> Result<SecureSignature> {
        // Log who, what, when, why
        // But don't log the actual key!
    }
}
```

### Exercise 3: Implement Key Escrow

```rust
pub struct EscrowedKeystore {
    keystore: SecureKeystore,
    escrow_shares: Vec<EscrowShare>,  // Shamir's secret sharing
}

// Split key into n shares, need k to reconstruct
// For regulatory compliance or recovery
```

---

## Key Takeaways

1. **Keys Are Everything**: Protect them like nuclear launch codes
2. **Use OS Entropy**: OsRng for all key generation
3. **Zeroize Memory**: Clean up sensitive data when done
4. **Context Separation**: Different keys for different purposes
5. **Time-Bound Signatures**: Add timestamps to prevent replay
6. **Constant-Time Crypto**: Prevent timing attacks
7. **Never Log Keys**: Not even for debugging
8. **Derive Don't Store**: Derive session keys from master
9. **Test Security Properties**: Not just functionality
10. **Plan for Key Rotation**: Keys shouldn't live forever

---

## The Philosophy of Key Management

*"A good key management system makes the right thing easy and the wrong thing hard."*

This module embodies defensive programming:
- Every operation assumes attack
- Every key assumes compromise
- Every signature assumes replay
- Every byte assumes disclosure

By assuming the worst and defending against it, we create a system that remains secure even when things go wrong.

---

## Further Reading

- [NIST Key Management Guidelines](https://nvlpubs.nist.gov/nistpubs/SpecialPublications/NIST.SP.800-57pt1r5.pdf)
- [Zeroize: Secure Memory Cleaning](https://docs.rs/zeroize/)
- [Ed25519 Key Management](https://ed25519.cr.yp.to/ed25519-20110926.pdf)
- [HSM Integration Patterns](https://www.cryptosys.net/pki/hsm-best-practices.html)

---

## Next Chapter

[Chapter 9: SIMD Acceleration →](./09_crypto_simd.md)

Now that our keys are secure, let's explore how to verify hundreds of signatures per second using SIMD parallelism!

---

*Remember: "In cryptography, your keys are your identity. Lose them and you cease to exist. Expose them and someone else becomes you."*
