use std::io::{Read, Write};

use argon2::{Algorithm, Argon2, Params, Version};
use base64::{Engine as _, engine::general_purpose::STANDARD, engine::general_purpose::URL_SAFE_NO_PAD};
use chacha20poly1305::{
    Key, XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use flate2::{Compression, read::ZlibDecoder, write::ZlibEncoder};
use hmac::{Hmac, KeyInit as HmacKeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const KDF_MEMORY_KIB: u32 = 64 * 1024;
const KDF_ITERATIONS: u32 = 3;
const KDF_PARALLELISM: u32 = 1;
const KEY_LENGTH: usize = 32;
const NONCE_LENGTH: usize = 24;
const FORMAT_VERSION: u8 = 1;
const KEY_AAD: &[u8] = b"dainiki-sync-key-v1";
const RECOVERY_AAD: &[u8] = b"dainiki-sync-recovery-v1";
const MANIFEST_AAD: &[u8] = b"dainiki-manifest-v1";

pub struct SyncCryptoError(pub String);

impl std::fmt::Debug for SyncCryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::fmt::Display for SyncCryptoError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for SyncCryptoError {}

pub type Result<T> = std::result::Result<T, SyncCryptoError>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SyncManifest {
    pub version: u8,
    pub updated_at: i64,
    pub entries: Vec<ManifestEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ManifestEntry {
    pub date: String,
    #[serde(default)]
    pub sync_id: String,
    pub hash: String,
    pub size: u64,
    pub revision: i64,
    #[serde(default)]
    pub deleted: bool,
}

#[derive(Debug, Clone)]
pub struct SyncKeys {
    pub salt: String,
    pub wrapped_key: String,
    pub recovery_wrapped_key: String,
    pub recovery_key: String,
    pub master_key: [u8; KEY_LENGTH],
}

pub fn random_bytes<const N: usize>() -> Result<[u8; N]> {
    let mut bytes = [0_u8; N];
    getrandom::fill(&mut bytes).map_err(|error| SyncCryptoError(error.to_string()))?;
    Ok(bytes)
}

pub fn random_salt() -> Result<String> {
    Ok(STANDARD.encode(random_bytes::<16>()?))
}

pub fn random_recovery_key() -> Result<String> {
    Ok(URL_SAFE_NO_PAD.encode(random_bytes::<32>()?))
}

pub fn recovery_key_bytes(recovery_key: &str) -> Result<[u8; KEY_LENGTH]> {
    let bytes = URL_SAFE_NO_PAD
        .decode(recovery_key.trim())
        .map_err(|_| SyncCryptoError("That recovery key is not valid.".to_string()))?;

    bytes
        .try_into()
        .map_err(|_| SyncCryptoError("That recovery key is not valid.".to_string()))
}

pub fn derive_passphrase_key(passphrase: &str, salt: &str) -> Result<[u8; KEY_LENGTH]> {
    let salt = STANDARD
        .decode(salt)
        .map_err(|_| SyncCryptoError("The sync salt is not valid.".to_string()))?;
    let params = Params::new(
        KDF_MEMORY_KIB,
        KDF_ITERATIONS,
        KDF_PARALLELISM,
        Some(KEY_LENGTH),
    )
    .map_err(|error| SyncCryptoError(error.to_string()))?;
    let argon2 = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = [0_u8; KEY_LENGTH];

    argon2
        .hash_password_into(passphrase.as_bytes(), &salt, &mut key)
        .map_err(|error| SyncCryptoError(error.to_string()))?;

    Ok(key)
}

pub fn create_sync_keys(passphrase: &str) -> Result<SyncKeys> {
    let salt = random_salt()?;
    let master_key: [u8; KEY_LENGTH] = random_bytes()?;
    let recovery_key = random_recovery_key()?;
    let passphrase_key = derive_passphrase_key(passphrase, &salt)?;
    let recovery_bytes = recovery_key_bytes(&recovery_key)?;
    let wrapped_key = wrap_key(&master_key, &passphrase_key, KEY_AAD)?;
    let recovery_wrapped_key = wrap_key(&master_key, &recovery_bytes, RECOVERY_AAD)?;

    Ok(SyncKeys {
        salt,
        wrapped_key,
        recovery_wrapped_key,
        recovery_key,
        master_key,
    })
}

pub fn unlock_with_passphrase(
    passphrase: &str,
    salt: &str,
    wrapped_key: &str,
) -> Result<[u8; KEY_LENGTH]> {
    let passphrase_key = derive_passphrase_key(passphrase, salt)?;
    unwrap_key(wrapped_key, &passphrase_key, KEY_AAD)
}

pub fn unlock_with_recovery_key(
    recovery_key: &str,
    recovery_wrapped_key: &str,
) -> Result<[u8; KEY_LENGTH]> {
    let recovery_bytes = recovery_key_bytes(recovery_key)?;
    unwrap_key(recovery_wrapped_key, &recovery_bytes, RECOVERY_AAD)
}

pub fn wrap_key(key: &[u8; KEY_LENGTH], wrapping_key: &[u8; KEY_LENGTH], aad: &[u8]) -> Result<String> {
    let sealed = seal(wrapping_key, aad, key)?;
    Ok(STANDARD.encode(sealed))
}

pub fn unwrap_key(
    wrapped: &str,
    wrapping_key: &[u8; KEY_LENGTH],
    aad: &[u8],
) -> Result<[u8; KEY_LENGTH]> {
    let bytes = STANDARD
        .decode(wrapped)
        .map_err(|_| SyncCryptoError("The wrapped sync key is not valid.".to_string()))?;
    let plaintext = open(wrapping_key, aad, &bytes)
        .map_err(|_| SyncCryptoError("That passphrase or recovery key is incorrect.".to_string()))?;
    plaintext
        .try_into()
        .map_err(|_| SyncCryptoError("The sync key has an invalid length.".to_string()))
}

pub fn encrypt_entry(
    key: &[u8; KEY_LENGTH],
    entry_id: &str,
    plaintext: &str,
) -> Result<Vec<u8>> {
    let compressed = compress(plaintext.as_bytes())?;
    seal(
        key,
        format!("dainiki-entry-v1:{entry_id}").as_bytes(),
        &compressed,
    )
}

pub fn decrypt_entry(
    key: &[u8; KEY_LENGTH],
    entry_id: &str,
    ciphertext: &[u8],
) -> Result<String> {
    let compressed = open(
        key,
        format!("dainiki-entry-v1:{entry_id}").as_bytes(),
        ciphertext,
    )?;
    let plaintext = decompress(&compressed)?;

    String::from_utf8(plaintext)
        .map_err(|_| SyncCryptoError("The decrypted entry is not valid UTF-8.".to_string()))
}

pub fn encrypt_manifest(key: &[u8; KEY_LENGTH], manifest: &SyncManifest) -> Result<Vec<u8>> {
    let json = serde_json::to_vec(manifest)
        .map_err(|error| SyncCryptoError(error.to_string()))?;
    let compressed = compress(&json)?;
    seal(key, MANIFEST_AAD, &compressed)
}

pub fn decrypt_manifest(key: &[u8; KEY_LENGTH], ciphertext: &[u8]) -> Result<SyncManifest> {
    let compressed = open(key, MANIFEST_AAD, ciphertext)?;
    let json = decompress(&compressed)?;

    serde_json::from_slice(&json).map_err(|error| SyncCryptoError(error.to_string()))
}

/// Deterministic opaque object id for a date. The server sees this id but
/// cannot recover the date without the master key.
pub fn sync_entry_id(key: &[u8; KEY_LENGTH], date: &str) -> String {
    let mut mac = <Hmac<Sha256> as HmacKeyInit>::new_from_slice(key)
        .expect("HMAC accepts keys of any length");
    mac.update(b"dainiki-entry-id-v1:");
    mac.update(date.as_bytes());
    hex_encode(&mac.finalize().into_bytes())
}

pub fn content_hash(plaintext: &str) -> String {
    bytes_hash(plaintext.as_bytes())
}

pub fn bytes_hash(bytes: &[u8]) -> String {
    hex_encode(&Sha256::digest(bytes))
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);

    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }

    output
}

fn seal(key: &[u8; KEY_LENGTH], aad: &[u8], plaintext: &[u8]) -> Result<Vec<u8>> {
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));
    let nonce_bytes: [u8; NONCE_LENGTH] = random_bytes()?;
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce_bytes),
            Payload {
                msg: plaintext,
                aad,
            },
        )
        .map_err(|_| SyncCryptoError("Could not encrypt sync data.".to_string()))?;

    let mut output = Vec::with_capacity(1 + NONCE_LENGTH + ciphertext.len());
    output.push(FORMAT_VERSION);
    output.extend_from_slice(&nonce_bytes);
    output.extend_from_slice(&ciphertext);

    Ok(output)
}

fn open(key: &[u8; KEY_LENGTH], aad: &[u8], sealed: &[u8]) -> Result<Vec<u8>> {
    if sealed.len() <= 1 + NONCE_LENGTH || sealed[0] != FORMAT_VERSION {
        return Err(SyncCryptoError("This encrypted sync data is not valid.".to_string()));
    }

    let nonce = &sealed[1..1 + NONCE_LENGTH];
    let ciphertext = &sealed[1 + NONCE_LENGTH..];
    let cipher = XChaCha20Poly1305::new(Key::from_slice(key));

    cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| SyncCryptoError("Could not decrypt sync data.".to_string()))
}

fn compress(input: &[u8]) -> Result<Vec<u8>> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(input)
        .map_err(|error| SyncCryptoError(error.to_string()))?;

    encoder
        .finish()
        .map_err(|error| SyncCryptoError(error.to_string()))
}

fn decompress(input: &[u8]) -> Result<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(input);
    let mut output = Vec::new();
    decoder
        .read_to_end(&mut output)
        .map_err(|error| SyncCryptoError(error.to_string()))?;

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn passphrase_derivation_is_deterministic() {
        let salt = STANDARD.encode([7_u8; 16]);
        let first = derive_passphrase_key("a long sync passphrase", &salt).unwrap();
        let second = derive_passphrase_key("a long sync passphrase", &salt).unwrap();
        let wrong = derive_passphrase_key("a different passphrase", &salt).unwrap();

        assert_eq!(first, second);
        assert_ne!(first, wrong);
    }

    #[test]
    fn wraps_and_unwraps_with_passphrase_and_recovery() {
        let keys = create_sync_keys("a long sync passphrase").unwrap();
        let passphrase_key =
            derive_passphrase_key("a long sync passphrase", &keys.salt).unwrap();
        let unlocked = unwrap_key(&keys.wrapped_key, &passphrase_key, KEY_AAD).unwrap();
        let recovered =
            unlock_with_recovery_key(&keys.recovery_key, &keys.recovery_wrapped_key).unwrap();

        assert_eq!(unlocked, keys.master_key);
        assert_eq!(recovered, keys.master_key);
    }

    #[test]
    fn rejects_wrong_keys() {
        let keys = create_sync_keys("a long sync passphrase").unwrap();
        assert!(unlock_with_passphrase(
            "the wrong passphrase",
            &keys.salt,
            &keys.wrapped_key
        )
        .is_err());
        assert!(unlock_with_recovery_key("not-a-recovery-key", &keys.recovery_wrapped_key).is_err());
    }

    #[test]
    fn encrypts_entries_without_leaking_plaintext() {
        let keys = create_sync_keys("a long sync passphrase").unwrap();
        let text = "A quiet morning with tea. #morning";
        let encrypted = encrypt_entry(&keys.master_key, "2026-10-23", text).unwrap();

        assert!(!encrypted.windows(text.len()).any(|window| window == text.as_bytes()));
        assert_eq!(
            decrypt_entry(&keys.master_key, "2026-10-23", &encrypted).unwrap(),
            text
        );
        assert!(decrypt_entry(&keys.master_key, "2026-10-22", &encrypted).is_err());
    }

    #[test]
    fn encrypts_manifests() {
        let keys = create_sync_keys("a long sync passphrase").unwrap();
        let manifest = SyncManifest {
            version: 1,
            updated_at: 42,
            entries: vec![ManifestEntry {
                date: "2026-10-23".to_string(),
                sync_id: sync_entry_id(&keys.master_key, "2026-10-23"),
                hash: content_hash("hello"),
                size: 5,
                revision: 3,
                deleted: false,
            }],
        };
        let encrypted = encrypt_manifest(&keys.master_key, &manifest).unwrap();

        assert_eq!(
            decrypt_manifest(&keys.master_key, &encrypted).unwrap(),
            manifest
        );
    }

    #[test]
    fn entry_ids_are_stable_opaque_and_date_specific() {
        let keys = create_sync_keys("a long sync passphrase").unwrap();
        let first = sync_entry_id(&keys.master_key, "2026-10-23");
        let second = sync_entry_id(&keys.master_key, "2026-10-23");
        let other = sync_entry_id(&keys.master_key, "2026-10-22");

        assert_eq!(first, second);
        assert_ne!(first, other);
        assert_eq!(first.len(), 64);
        assert!(!first.contains("2026"));
    }

    #[test]
    fn content_hash_is_stable() {
        assert_eq!(content_hash("hello"), content_hash("hello"));
        assert_ne!(content_hash("hello"), content_hash("hello!"));
    }
}
