//! Simple VRF interface (placeholder for real VRF implementation)

use crate::error::Result;

#[derive(Clone)]
pub struct VRFPublicKey(pub [u8; 32]);

#[derive(Clone)]
pub struct VRFSecretKey(pub [u8; 32]);

#[derive(Clone)]
pub struct VRFKeypair {
    pub pk: VRFPublicKey,
    pub sk: VRFSecretKey,
}

#[derive(Clone)]
pub struct VRFOutput(pub [u8; 32]);

#[derive(Clone)]
pub struct VRFProof(pub Vec<u8>);

impl VRFKeypair {
    /// Deterministic test generator; replace with real keygen
    pub fn generate_deterministic(seed: u64) -> Self {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(seed.to_le_bytes());
        let digest = hasher.finalize();
        let mut sk = [0u8; 32];
        sk.copy_from_slice(&digest[..32]);
        let mut pk = [0u8; 32];
        // Fake pk = hash(sk)
        let mut h2 = Sha256::new();
        h2.update(&sk);
        let d2 = h2.finalize();
        pk.copy_from_slice(&d2[..32]);
        Self { pk: VRFPublicKey(pk), sk: VRFSecretKey(sk) }
    }
}

/// Produce VRF output and proof for given input
pub fn prove(_sk: &VRFSecretKey, input: &[u8]) -> Result<(VRFOutput, VRFProof)> {
    use sha2::{Digest, Sha256};
    // Placeholder: output = sha256(input), proof = input bytes
    let mut hasher = Sha256::new();
    hasher.update(input);
    let digest = hasher.finalize();
    let mut out = [0u8; 32];
    out.copy_from_slice(&digest[..32]);
    Ok((VRFOutput(out), VRFProof(input.to_vec())))
}

/// Verify VRF proof
pub fn verify(_pk: &VRFPublicKey, input: &[u8], out: &VRFOutput, proof: &VRFProof) -> bool {
    if proof.0 != input { return false; }
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(input);
    let digest = hasher.finalize();
    out.0 == digest[..32]
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_vrf_stub_roundtrip() {
        let kp = VRFKeypair::generate_deterministic(42);
        let input = b"round-1";
        let (out, proof) = prove(&kp.sk, input).unwrap();
        assert!(verify(&kp.pk, input, &out, &proof));
        assert!(!verify(&kp.pk, b"different", &out, &proof));
    }
}

