//! Signed intent binding.
//!
//! The agent signs exactly what it wants to do, including an expiry. The contract
//! rebuilds the same message and verifies the signature, so a compromised host
//! cannot change the recipient or amount between "agent decided" and "contract
//! executed", and cannot sit on an old signature until conditions change.

pub const DOMAIN: &[u8] = b"AGENT_GUARD_INTENT_V2";

pub fn message(
    contract: &str,
    recipient: &str,
    amount: u128,
    nonce: u64,
    valid_until: u64,
) -> Vec<u8> {
    let mut m = Vec::with_capacity(DOMAIN.len() + contract.len() + recipient.len() + 40);
    m.extend_from_slice(DOMAIN);
    m.push(0);
    m.extend_from_slice(contract.as_bytes());
    m.push(0);
    m.extend_from_slice(recipient.as_bytes());
    m.push(0);
    m.extend_from_slice(&amount.to_le_bytes());
    m.extend_from_slice(&nonce.to_le_bytes());
    m.extend_from_slice(&valid_until.to_le_bytes());
    m
}

#[cfg(target_arch = "wasm32")]
pub fn verify(pubkey: &[u8], message: &[u8], signature: &[u8]) -> bool {
    if pubkey.len() != 32 || signature.len() != 64 {
        return false;
    }
    let mut pk = [0u8; 32];
    pk.copy_from_slice(pubkey);
    let mut sig = [0u8; 64];
    sig.copy_from_slice(signature);
    near_sdk::env::ed25519_verify(&sig, message, &pk)
}

#[cfg(not(target_arch = "wasm32"))]
pub fn verify(pubkey: &[u8], message: &[u8], signature: &[u8]) -> bool {
    use ed25519_dalek::{Signature, VerifyingKey};

    if pubkey.len() != 32 || signature.len() != 64 {
        return false;
    }
    let mut pk = [0u8; 32];
    pk.copy_from_slice(pubkey);
    let Ok(vk) = VerifyingKey::from_bytes(&pk) else {
        return false;
    };
    let mut sb = [0u8; 64];
    sb.copy_from_slice(signature);
    vk.verify_strict(message, &Signature::from_bytes(&sb)).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};

    fn key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    const CONTRACT: &str = "guard.testnet";
    const TO: &str = "merchant.near";
    const AMOUNT: u128 = 500;
    const NONCE: u64 = 1;
    const UNTIL: u64 = 1_700_000_300;

    #[test]
    fn valid_signature_passes() {
        let msg = message(CONTRACT, TO, AMOUNT, NONCE, UNTIL);
        let sig = key().sign(&msg);
        assert!(verify(&key().verifying_key().to_bytes(), &msg, &sig.to_bytes()));
    }

    #[test]
    fn wrong_recipient_fails() {
        let msg = message(CONTRACT, TO, AMOUNT, NONCE, UNTIL);
        let sig = key().sign(&msg);
        let tampered = message(CONTRACT, "attacker.near", AMOUNT, NONCE, UNTIL);
        assert!(!verify(&key().verifying_key().to_bytes(), &tampered, &sig.to_bytes()));
    }

    #[test]
    fn wrong_amount_nonce_or_expiry_fails() {
        let msg = message(CONTRACT, TO, AMOUNT, NONCE, UNTIL);
        let sig = key().sign(&msg);
        let pk = key().verifying_key().to_bytes();
        assert!(!verify(&pk, &message(CONTRACT, TO, AMOUNT + 1, NONCE, UNTIL), &sig.to_bytes()));
        assert!(!verify(&pk, &message(CONTRACT, TO, AMOUNT, NONCE + 1, UNTIL), &sig.to_bytes()));
        assert!(!verify(&pk, &message(CONTRACT, TO, AMOUNT, NONCE, UNTIL + 1), &sig.to_bytes()));
    }

    #[test]
    fn malformed_inputs_fail() {
        let msg = message(CONTRACT, TO, AMOUNT, NONCE, UNTIL);
        assert!(!verify(&[0u8; 31], &msg, &[0u8; 64]));
        assert!(!verify(&[0u8; 32], &msg, &[0u8; 63]));
    }
}
