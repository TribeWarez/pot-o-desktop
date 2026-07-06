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
    use rand::RngCore;
    let mut secret = [0u8; 32];
    OsRng.fill_bytes(&mut secret);
    let secret_key = ed25519_dalek::SecretKey::from(secret);
    let signing_key = SigningKey::from_bytes(&secret_key);
    let pubkey = hex::encode(signing_key.verifying_key().as_bytes());

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

/// Extract pubkey from a keypair file.
///
/// Recognises three formats:
/// - 64 bytes: full keypair (secret || pubkey)
/// - 32 bytes: either a raw public key, or a secret key (from which we derive the pubkey)
/// - anything else: hex-encoded verbatim
pub fn pubkey_from_file(path: &str) -> Result<KeypairInfo, String> {
    let content = std::fs::read_to_string(path).map_err(|e| format!("Cannot read {}: {}", path, e))?;
    let bytes: Vec<u8> =
        serde_json::from_str(&content).map_err(|_| "Not a valid JSON array".to_string())?;

    if bytes.len() == 64 {
        // Full keypair format — bytes [0..32] are secret, bytes [32..64] are pubkey
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
        let arr: [u8; 32] = bytes[..32]
            .try_into()
            .map_err(|_| "Invalid byte array length".to_string())?;

        // First, try as a raw public key
        if let Ok(vk) = VerifyingKey::from_bytes(&arr) {
            return Ok(KeypairInfo {
                pubkey: hex::encode(vk.as_bytes()),
                path: path.to_string(),
                exists: true,
                is_keypair: false,
            });
        }

        // Fallback: treat as a secret key and derive the pubkey.
        // This handles files written by our own generate_keypair.
        let secret_key = ed25519_dalek::SecretKey::from(arr);
        let signing_key = SigningKey::from_bytes(&secret_key);
        let pubkey = hex::encode(signing_key.verifying_key().as_bytes());
        Ok(KeypairInfo {
            pubkey,
            path: path.to_string(),
            exists: true,
            is_keypair: true,
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

/// Detect if a file is a 64-byte full keypair (secret || pubkey)
pub fn is_full_keypair(path: &str) -> bool {
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

#[cfg(test)]
mod tests {
    use super::*;
    use rand::RngCore;
    use std::fs;

    fn create_signing_key() -> SigningKey {
        let mut secret = [0u8; 32];
        OsRng.fill_bytes(&mut secret);
        let secret_key = ed25519_dalek::SecretKey::from(secret);
        SigningKey::from_bytes(&secret_key)
    }

    #[test]
    fn test_generate_and_read_keypair() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test-keypair.json");
        let path_str = path.to_str().unwrap();

        let info = generate_keypair(path_str).unwrap();
        assert_eq!(info.path, path_str);
        assert!(info.exists);
        assert!(info.is_keypair);
        assert_eq!(info.pubkey.len(), 64); // 32 bytes = 64 hex chars

        let read_info = pubkey_from_file(path_str).unwrap();
        assert_eq!(read_info.pubkey, info.pubkey);
        assert!(read_info.is_keypair);
    }

    #[test]
    fn test_read_non_existent_file() {
        let result = pubkey_from_file("/nonexistent/path/keypair.json");
        assert!(result.is_err());
    }

    #[test]
    fn test_is_full_keypair_true() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("solana-kp.json");
        let path_str = path.to_str().unwrap();

        // Create a 64-byte keypair
        let kp = create_signing_key();
        let bytes: Vec<u8> = kp.to_bytes().to_vec();
        assert_eq!(bytes.len(), 64);
        let json = serde_json::to_string(&bytes).unwrap();
        fs::write(path_str, &json).unwrap();

        assert!(is_full_keypair(path_str));
    }

    #[test]
    fn test_is_full_keypair_false() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not-keypair.json");
        let path_str = path.to_str().unwrap();

        // 32-byte array is not a full keypair
        let bytes: Vec<u8> = vec![0u8; 32];
        let json = serde_json::to_string(&bytes).unwrap();
        fs::write(path_str, &json).unwrap();

        assert!(!is_full_keypair(path_str));
    }

    #[test]
    fn test_is_full_keypair_missing_file() {
        assert!(!is_full_keypair("/nonexistent/path.json"));
    }

    #[test]
    fn test_is_full_keypair_invalid_json() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("invalid.json");
        let path_str = path.to_str().unwrap();
        fs::write(path_str, "not json").unwrap();
        assert!(!is_full_keypair(path_str));
    }

    #[test]
    fn test_read_arbitrary_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("arb.json");
        let path_str = path.to_str().unwrap();

        let bytes: Vec<u8> = vec![1, 2, 3, 4, 5];
        let json = serde_json::to_string(&bytes).unwrap();
        fs::write(path_str, &json).unwrap();

        let info = pubkey_from_file(path_str).unwrap();
        assert!(!info.is_keypair);
        assert_eq!(info.pubkey, hex::encode(&bytes));
    }

    #[test]
    fn test_read_raw_32byte_pubkey() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pubkey.json");
        let path_str = path.to_str().unwrap();

        let kp = create_signing_key();
        let pubkey_bytes = kp.verifying_key().as_bytes().to_vec();
        assert_eq!(pubkey_bytes.len(), 32);
        let json = serde_json::to_string(&pubkey_bytes).unwrap();
        fs::write(path_str, &json).unwrap();

        let info = pubkey_from_file(path_str).unwrap();
        assert!(!info.is_keypair);
        assert_eq!(info.pubkey, hex::encode(&pubkey_bytes));
    }

    #[test]
    fn test_generated_keypair_can_be_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kp.json");
        let path_str = path.to_str().unwrap();
        let gen = generate_keypair(path_str).unwrap();

        // Generated file is 32 bytes (secret key only), not 64
        assert!(!is_full_keypair(path_str));

        // But it must load back successfully via the secret-key fallback
        let loaded = pubkey_from_file(path_str).unwrap();
        assert_eq!(loaded.pubkey, gen.pubkey);
        assert!(loaded.is_keypair);
    }

    #[test]
    fn test_secret_key_bytes_load_as_keypair() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("secret.json");
        let path_str = path.to_str().unwrap();

        // Write just the 32-byte secret key
        let sk = create_signing_key();
        let secret_bytes: Vec<u8> = sk.to_bytes().to_vec();
        assert_eq!(secret_bytes.len(), 32);
        let json = serde_json::to_string(&secret_bytes).unwrap();
        fs::write(path_str, &json).unwrap();

        let info = pubkey_from_file(path_str).unwrap();
        assert!(info.is_keypair);
        assert_eq!(info.pubkey, hex::encode(sk.verifying_key().as_bytes()));
    }
}
