//! Prints an ed25519 signature for an Agent Guard intent, plus the agent
//! public key.
//!
//! The key comes from `AGENT_SEED` (64 hex chars) when set; otherwise a random
//! key is generated. Tests use a fixed seed. Never reuse a production key here.
//!
//! Usage:
//!   cargo run --example agent_sign -- <contract> <recipient> <amount> <nonce> <valid_until>

use ed25519_dalek::{Signer, SigningKey};
use rand_core::OsRng;

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 6 {
        eprintln!(
            "usage: agent_sign <contract> <recipient> <amount_yocto> <nonce> <valid_until>"
        );
        std::process::exit(2);
    }
    let contract = &args[1];
    let recipient = &args[2];
    let amount: u128 = args[3].parse().expect("amount must be an integer");
    let nonce: u64 = args[4].parse().expect("nonce must be an integer");
    let valid_until: u64 = args[5].parse().expect("valid_until must be an integer");

    let key = match std::env::var("AGENT_SEED") {
        Ok(seed) if seed.len() == 64 => {
            let bytes = (0..32)
                .map(|i| u8::from_str_radix(&seed[i * 2..i * 2 + 2], 16).expect("bad seed"))
                .collect::<Vec<u8>>();
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&bytes);
            SigningKey::from_bytes(&arr)
        }
        _ => SigningKey::generate(&mut OsRng),
    };

    let msg = agent_guard::intent::message(contract, recipient, amount, nonce, valid_until);
    let sig = key.sign(&msg);

    println!("pubkey={}", hex(key.verifying_key().as_bytes()));
    println!("signature={}", hex(&sig.to_bytes()));
}
