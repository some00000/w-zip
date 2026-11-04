use aes_gcm::aead::generic_array::GenericArray;
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::Aes256Gcm;
use argon2::{Algorithm, Argon2, Params, Version};
use rand::RngCore;
use zeroize::Zeroize;

use crate::error::Error;

pub const MAGIC: &[u8; 4] = b"WZIP";
pub const VERSION: u8 = 1;
const SALT_LEN: usize = 16;
const NONCE_LEN: usize = 12;
const KEY_LEN: usize = 32;
const TAG_LEN: usize = 16;
const PREFIX_LEN: usize = 4 + 1 + 4 + 4 + 4 + SALT_LEN;
pub const HEADER_LEN: usize = PREFIX_LEN + NONCE_LEN;

/// Interactive Argon2id parameters (OWASP: 19 MiB, 2 iterations, 1 lane).
pub const INTERACTIVE_MEMORY_KIB: u32 = 19_456;
pub const INTERACTIVE_TIME: u32 = 2;
pub const INTERACTIVE_PARALLELISM: u32 = 1;

const MIN_MEMORY_KIB: u32 = 8;
const MAX_MEMORY_KIB: u32 = 1_048_576;
const MIN_TIME: u32 = 1;
const MAX_TIME: u32 = 10;
const MIN_LANES: u32 = 1;
const MAX_LANES: u32 = 8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KdfParams {
    pub memory_kib: u32,
    pub time_cost: u32,
    pub parallelism: u32,
}

impl KdfParams {
    pub fn interactive() -> Self {
        Self {
            memory_kib: INTERACTIVE_MEMORY_KIB,
            time_cost: INTERACTIVE_TIME,
            parallelism: INTERACTIVE_PARALLELISM,
        }
    }

    /// Fast parameters for tests. Do not use these for archives a person will keep.
    #[cfg(test)]
    pub fn minimal() -> Self {
        Self {
            memory_kib: MIN_MEMORY_KIB,
            time_cost: MIN_TIME,
            parallelism: MIN_LANES,
        }
    }
}

struct DerivedKey([u8; KEY_LEN]);

impl Drop for DerivedKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

/// Encrypt `plaintext` (the zip payload) into a `.wzip` blob.
pub fn seal(plaintext: &[u8], password: &str, kdf: KdfParams) -> Result<Vec<u8>, Error> {
    if password.is_empty() {
        return Err(Error::EmptyPassword);
    }
    validate_params(kdf)?;

    let mut salt = [0u8; SALT_LEN];
    let mut nonce_bytes = [0u8; NONCE_LEN];
    rand::rngs::OsRng.fill_bytes(&mut salt);
    rand::rngs::OsRng.fill_bytes(&mut nonce_bytes);

    let mut header = Vec::with_capacity(HEADER_LEN + plaintext.len() + TAG_LEN);
    header.extend_from_slice(MAGIC);
    header.push(VERSION);
    header.extend_from_slice(&kdf.memory_kib.to_le_bytes());
    header.extend_from_slice(&kdf.time_cost.to_le_bytes());
    header.extend_from_slice(&kdf.parallelism.to_le_bytes());
    header.extend_from_slice(&salt);
    debug_assert_eq!(header.len(), PREFIX_LEN);

    let key = derive_key(password, &salt, kdf)?;
    let cipher = Aes256Gcm::new_from_slice(&key.0).map_err(|_| Error::Decrypt)?;
    let nonce = GenericArray::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(
            nonce,
            Payload {
                msg: plaintext,
                aad: &header,
            },
        )
        .map_err(|_| Error::Decrypt)?;

    header.extend_from_slice(&nonce_bytes);
    header.extend_from_slice(&ciphertext);
    Ok(header)
}

/// Decrypt a `.wzip` blob. The password check and the header binding both
/// come from the AES-GCM tag.
pub fn open(archive: &[u8], password: &str) -> Result<Vec<u8>, Error> {
    if password.is_empty() {
        return Err(Error::EmptyPassword);
    }
    if archive.get(..4) != Some(MAGIC.as_slice()) {
        return Err(if archive.len() < 4 {
            Error::Truncated
        } else {
            Error::BadMagic
        });
    }
    if archive.len() < HEADER_LEN + TAG_LEN {
        return Err(Error::Truncated);
    }
    let version = archive[4];
    if version != VERSION {
        return Err(Error::UnsupportedVersion(version));
    }

    let memory_kib = read_u32(archive, 5)?;
    let time_cost = read_u32(archive, 9)?;
    let parallelism = read_u32(archive, 13)?;
    let kdf = KdfParams {
        memory_kib,
        time_cost,
        parallelism,
    };
    validate_params(kdf)?;

    let salt = &archive[17..17 + SALT_LEN];
    let nonce_bytes = &archive[PREFIX_LEN..HEADER_LEN];
    let ciphertext = &archive[HEADER_LEN..];
    let aad = &archive[..PREFIX_LEN];

    let key = derive_key(password, salt, kdf)?;
    let cipher = Aes256Gcm::new_from_slice(&key.0).map_err(|_| Error::Decrypt)?;
    let nonce = GenericArray::from_slice(nonce_bytes);
    cipher
        .decrypt(
            nonce,
            Payload {
                msg: ciphertext,
                aad,
            },
        )
        .map_err(|_| Error::Decrypt)
}

fn validate_params(kdf: KdfParams) -> Result<(), Error> {
    if !(MIN_MEMORY_KIB..=MAX_MEMORY_KIB).contains(&kdf.memory_kib)
        || !(MIN_TIME..=MAX_TIME).contains(&kdf.time_cost)
        || !(MIN_LANES..=MAX_LANES).contains(&kdf.parallelism)
        || kdf.memory_kib < kdf.parallelism.saturating_mul(8)
    {
        return Err(Error::InvalidParams);
    }
    Params::new(
        kdf.memory_kib,
        kdf.time_cost,
        kdf.parallelism,
        Some(KEY_LEN),
    )
    .map(|_| ())
    .map_err(|_| Error::InvalidParams)
}

fn derive_key(password: &str, salt: &[u8], kdf: KdfParams) -> Result<DerivedKey, Error> {
    let params = Params::new(
        kdf.memory_kib,
        kdf.time_cost,
        kdf.parallelism,
        Some(KEY_LEN),
    )
    .map_err(|_| Error::InvalidParams)?;
    let argon = Argon2::new(Algorithm::Argon2id, Version::V0x13, params);
    let mut key = DerivedKey([0u8; KEY_LEN]);
    argon
        .hash_password_into(password.as_bytes(), salt, &mut key.0)
        .map_err(|_| Error::InvalidParams)?;
    Ok(key)
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32, Error> {
    let bytes: [u8; 4] = data
        .get(offset..offset + 4)
        .ok_or(Error::Truncated)?
        .try_into()
        .map_err(|_| Error::Truncated)?;
    Ok(u32::from_le_bytes(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_header_layout() {
        let plain = b"zip-bytes-go-here";
        let sealed = seal(plain, "correct horse", KdfParams::minimal()).unwrap();
        assert!(sealed.starts_with(b"WZIP"));
        assert_eq!(sealed[4], VERSION);
        assert_eq!(read_u32(&sealed, 5).unwrap(), MIN_MEMORY_KIB);
        assert_eq!(read_u32(&sealed, 9).unwrap(), MIN_TIME);
        assert_eq!(read_u32(&sealed, 13).unwrap(), MIN_LANES);
        assert_eq!(open(&sealed, "correct horse").unwrap(), plain);
    }

    #[test]
    fn wrong_password_and_tamper_fail() {
        let sealed = seal(b"payload", "secret", KdfParams::minimal()).unwrap();
        assert!(matches!(
            open(&sealed, "nope"),
            Err(Error::Decrypt)
        ));

        let mut flipped = sealed.clone();
        let last = flipped.len() - 1;
        flipped[last] ^= 0x01;
        assert!(matches!(open(&flipped, "secret"), Err(Error::Decrypt)));

        let mut header = sealed.clone();
        header[5] ^= 0x01;
        assert!(matches!(open(&header, "secret"), Err(Error::Decrypt) | Err(Error::InvalidParams)));
    }

    #[test]
    fn rejects_empty_password_bad_magic_and_truncation() {
        assert!(matches!(
            seal(b"x", "", KdfParams::minimal()),
            Err(Error::EmptyPassword)
        ));
        assert!(matches!(open(b"WZIP", "pw"), Err(Error::Truncated)));
        assert!(matches!(open(b"NOPE", "pw"), Err(Error::BadMagic)));

        let mut sealed = seal(b"x", "pw", KdfParams::minimal()).unwrap();
        sealed[4] = 9;
        assert!(matches!(
            open(&sealed, "pw"),
            Err(Error::UnsupportedVersion(9))
        ));
    }

    #[test]
    fn rejects_hostile_kdf_memory_before_hashing() {
        let mut blob = vec![0u8; HEADER_LEN + TAG_LEN];
        blob[..4].copy_from_slice(b"WZIP");
        blob[4] = VERSION;
        blob[5..9].copy_from_slice(&u32::MAX.to_le_bytes());
        blob[9..13].copy_from_slice(&1u32.to_le_bytes());
        blob[13..17].copy_from_slice(&1u32.to_le_bytes());
        assert!(matches!(open(&blob, "pw"), Err(Error::InvalidParams)));
    }

    #[test]
    fn plaintext_is_not_visible_in_the_blob() {
        let marker = b"super-secret-marker-9f3a";
        let sealed = seal(marker, "pw", KdfParams::minimal()).unwrap();
        assert!(!sealed.windows(marker.len()).any(|window| window == marker));
    }
}
