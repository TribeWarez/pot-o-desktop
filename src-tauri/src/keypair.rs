use ed25519_dalek::{SigningKey, VerifyingKey};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeypairInfo {
    pub pubkey: String,
    pub path: String,
    pub exists: bool,
    pub is_keypair: bool,
}

/// Generate a new Ed25519 keypair, save to path, return pubkey
pub fn generate_keypair(path: &str) -> Result<KeypairInfo, String> {
    let mut secret = [0u8; 32];
    use rand::RngCore;
    OsRng.fill_bytes(&mut secret);

    let secret_key = ed25519_dalek::SecretKey::from(secret);
    let signing_key = SigningKey::from_bytes(&secret_key);
    let verifying_key = signing_key.verifying_key();
    let pubkey = hex::encode(verifying_key.as_bytes());

    let vec: Vec<u8> = signing_key.to_bytes().to_vec();
    let json = serde_json::to_string(&vec).map_err(|e| e.to_string())?;
    std::fs::write(path, &json).map_err(|e| e.to_string())?;

    Ok(KeypairInfo {
        pubkey,
        path: path.to_string(),
        exists: true,
        is_keypair: true,
    })
}

/// Extract pubkey from a keypair file
pub fn pubkey_from_file(path: &str) -> Result<KeypairInfo, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("Cannot read {}: {}", path, e))?;
    let bytes: Vec<u8> =
        serde_json::from_str(&content).map_err(|_| "Not a valid JSON array".to_string())?;

    if bytes.len() == 64 {
        // Solana keypair format — bytes [0..32] are secret, bytes [32..64] are pubkey
        // We can derive pubkey from secret
        let kp_bytes: [u8; 64] = bytes[..64]
            .try_into()
            .map_err(|_| "Invalid keypair length".to_string())?;
        let signing_key =
            SigningKey::from_keypair_bytes(&kp_bytes).map_err(|e| format!("Invalid keypair: {}", e))?;
        let pubkey = hex::encode(signing_key.verifying_key().as_bytes());
        Ok(KeypairInfo {
            pubkey,
            path: path.to_string(),
            exists: true,
            is_keypair: true,
        })
    } else if bytes.len() == 32 {
        // Raw 32-byte public key
        let pk_bytes: [u8; 32] = bytes[..32]
            .try_into()
            .map_err(|_| "Invalid pubkey length".to_string())?;
        let verifying_key =
            VerifyingKey::from_bytes(&pk_bytes).map_err(|e| format!("Invalid pubkey: {}", e))?;
        let pubkey = hex::encode(verifying_key.as_bytes());
        Ok(KeypairInfo {
            pubkey,
            path: path.to_string(),
            exists: true,
            is_keypair: false,
        })
    } else {
        Ok(KeypairInfo {
            pubkey: hex::encode(&bytes),
            path: path.to_string(),
            exists: true,
            is_keypair: false,
        })
    }
}

/// Detect if a file is a 64-byte Solana keypair
pub fn is_solana_keypair(path: &str) -> bool {
    if !Path::new(path).exists() {
        return false;
    }
    let content = std::fs::read_to_string(path).ok();
    match content {
        Some(c) => serde_json::from_str::<Vec<u8>>(&c)
            .map(|v| v.len() == 64)
            .unwrap_or(false),
        None => false,
    }
}
