//! `.sav` container: AES-256-ECB over PKCS7(zlib(yaml) + u32le(len(yaml))).
//!
//! The key is a fixed base key whose first 8 bytes are XORed with the Steam ID
//! (u64 little-endian). The game writes zlib level 6 with a 4-byte length
//! trailer; older tools wrote level 9 with adler32 + length. Both decrypt.
//! Writing uses the game's form; with the C zlib backend the output is
//! byte-identical to what the game writes.

use aes::cipher::{generic_array::GenericArray, BlockDecrypt, BlockEncrypt, KeyInit};
use aes::Aes256;
use flate2::{read::ZlibDecoder, write::ZlibEncoder, Compression};
use std::io::{Read, Write};

const BASE_KEY: [u8; 32] = [
    0x35, 0xEC, 0x33, 0x77, 0xF3, 0x5D, 0xB0, 0xEA, 0xBE, 0x6B, 0x83, 0x11, 0x54, 0x03, 0xEB, 0xFB,
    0x27, 0x25, 0x64, 0x2E, 0xD5, 0x49, 0x06, 0x29, 0x05, 0x78, 0xBD, 0x60, 0xBA, 0x4A, 0xA7, 0x87,
];

#[derive(Debug, thiserror::Error)]
pub enum CryptoError {
    #[error("file size {0} is not a multiple of 16 (not a BL4 save?)")]
    BadSize(usize),
    #[error("decryption failed: wrong Steam ID or not a BL4 save")]
    BadData,
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

pub fn derive_key(steam_id: u64) -> [u8; 32] {
    let mut k = BASE_KEY;
    for (i, b) in steam_id.to_le_bytes().iter().enumerate() {
        k[i] ^= b;
    }
    k
}

pub fn decrypt(data: &[u8], steam_id: u64) -> Result<Vec<u8>, CryptoError> {
    if data.is_empty() || data.len() % 16 != 0 {
        return Err(CryptoError::BadSize(data.len()));
    }
    let key = derive_key(steam_id);
    let cipher = Aes256::new(GenericArray::from_slice(&key));
    let mut buf = data.to_vec();
    for block in buf.chunks_mut(16) {
        cipher.decrypt_block(GenericArray::from_mut_slice(block));
    }
    // PKCS7
    let pad = *buf.last().unwrap() as usize;
    if (1..=16).contains(&pad) && buf[buf.len() - pad..].iter().all(|&b| b as usize == pad) {
        buf.truncate(buf.len() - pad);
    }
    if buf.len() < 2 || buf[0] != 0x78 {
        return Err(CryptoError::BadData);
    }
    let mut out = Vec::new();
    ZlibDecoder::new(&buf[..]).read_to_end(&mut out).map_err(|_| CryptoError::BadData)?;
    Ok(out)
}

pub fn encrypt(yaml: &[u8], steam_id: u64) -> Result<Vec<u8>, CryptoError> {
    let mut enc = ZlibEncoder::new(Vec::new(), Compression::new(6));
    enc.write_all(yaml)?;
    let mut buf = enc.finish()?;
    buf.extend_from_slice(&(yaml.len() as u32).to_le_bytes());
    let pad = 16 - (buf.len() % 16);
    buf.extend(std::iter::repeat(pad as u8).take(pad));
    let key = derive_key(steam_id);
    let cipher = Aes256::new(GenericArray::from_slice(&key));
    for block in buf.chunks_mut(16) {
        cipher.encrypt_block(GenericArray::from_mut_slice(block));
    }
    Ok(buf)
}

/// Try to read the Steam ID from a save path: the save folder is
/// `SaveGames\<steamid>\Profiles\client\N.sav`.
pub fn steam_id_from_path(p: &std::path::Path) -> Option<u64> {
    for anc in p.ancestors() {
        if let Some(name) = anc.file_name().and_then(|n| n.to_str()) {
            if name.len() == 17 && name.starts_with("7656") {
                if let Ok(v) = name.parse() {
                    return Some(v);
                }
            }
        }
    }
    None
}
