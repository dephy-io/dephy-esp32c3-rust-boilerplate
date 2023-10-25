use crate::preludes::*;
use esp_idf_sys::esp_fill_random;
use k256::ecdsa::{RecoveryId, Signature, SigningKey, VerifyingKey};
use k256::SecretKey;
use lazy_static::lazy_static;
use sha3::{Digest, Keccak256};
use std::ffi::c_void;

lazy_static! {
    pub static ref SECRET_KEY: SecretKey = get_device_secret_key().unwrap();
    pub static ref MY_ADDRESS_BYTES: [u8; 20] =
        get_eth_address_bytes(&SECRET_KEY.public_key().into());
    pub static ref MY_ADDRESS_STRING: String = hex::encode(MY_ADDRESS_BYTES.as_slice());
}

pub fn get_device_secret_key() -> Result<SecretKey> {
    let buf = crate::key_inspect::get_key()?.ok_or(anyhow!("Key not provisionned"))?;
    Ok(SecretKey::from_slice(&buf)?)
}

#[allow(dead_code)]
pub fn get_random_key() -> Result<SecretKey> {
    let buf = unsafe {
        let mut buf = [0u8; 32];
        esp_fill_random(buf.as_mut_ptr() as *mut c_void, 32);
        buf
    };
    Ok(SecretKey::from_slice(&buf)?)
}

pub fn get_eth_address_bytes(key: &VerifyingKey) -> [u8; 20] {
    let key = key.to_encoded_point(false);
    let key = key.as_bytes();
    let mut hasher = Keccak256::default();
    hasher.update(&key[1..]);
    let hash: [u8; 32] = hasher.finalize().into();
    let addr = &hash[12..32];
    addr.try_into().unwrap()
}

pub fn get_eth_address(key: &VerifyingKey) -> String {
    format!("0x{}", hex::encode(get_eth_address_bytes(key)))
}

#[allow(dead_code)]
pub fn did_str_to_addr_bytes<T: Into<String>>(did_str: T) -> Result<Vec<u8>> {
    let did_str: String = did_str.into();
    let did_str = did_str
        .strip_prefix("did:dephy:0x")
        .ok_or(anyhow!("Not in DID string format."))?;
    if did_str.len() != 40 {
        bail!("Invalid length for an DID string format.")
    }
    Ok(hex::decode(did_str)?)
}

pub fn create_signed_message(
    payload: Vec<u8>,
    to_address: Option<Vec<u8>>,
) -> Result<SignedMessage> {
    let signer: SigningKey = SECRET_KEY.clone().into();
    let from_address = MY_ADDRESS_BYTES.to_vec();
    let time = Utc::now();
    let timestamp = time.timestamp() as u64;
    let raw = RawMessage {
        timestamp,
        from_address,
        to_address: if let Some(t) = to_address {
            t
        } else {
            [0u8; 20].into()
        },
        encrypted: false,
        payload,
        iv: None,
        w3b: None,
    };
    let raw = raw.encode_to_vec();
    let mut hasher = Keccak256::new();
    hasher.update(&raw);
    hasher.update(timestamp.to_string().as_bytes());
    let raw_hash = hasher.finalize_reset();
    hasher.update(&raw_hash);
    let (signature, recid) = signer.sign_digest_recoverable(hasher)?;
    let mut sign_bytes = signature.to_vec();
    sign_bytes.append(&mut vec![recid.to_byte()]);

    Ok(SignedMessage {
        raw,
        hash: raw_hash.to_vec(),
        nonce: timestamp,
        signature: sign_bytes,
        last_edge_addr: None,
    })
}

#[allow(dead_code)]
pub fn check_message(data: &[u8]) -> Result<(SignedMessage, RawMessage)> {
    ensure!(data.len() > 0, "Message should not be empty!");

    let mut hasher = Keccak256::new();

    let msg = SignedMessage::decode(data)?;
    let SignedMessage {
        raw,
        hash,
        nonce,
        signature,
        ..
    } = msg.clone();
    let raw = raw.as_slice();
    let hash = hash.as_slice();
    let hash_hex = hex::encode(hash);
    hasher.update(raw);
    hasher.update(nonce.to_string().as_bytes());
    let curr_hash = hasher.finalize_reset();
    ensure!(
        hash == curr_hash.as_slice(),
        "Hash verification failed: expected=0x{} current=0x{}",
        hash_hex,
        hex::encode(curr_hash)
    );
    debug!("Raw message hash: 0x{}", hash_hex);

    let raw_msg = RawMessage::decode(raw)?;
    let RawMessage {
        timestamp,
        from_address,
        ..
    } = raw_msg.clone();
    ensure!(
        nonce == timestamp,
        "Message timestamp check failed: outer={} inner={}",
        nonce,
        timestamp
    );

    let from_address = from_address.as_slice();
    let from_address_hex = hex::encode(from_address);
    let signature = signature.as_slice();
    ensure!(signature.len() == 65, "Bad signature length!");
    let r = &signature[0..32];
    let s = &signature[32..64];
    let v = &signature[64..];
    debug!(
        "R: 0x{}\nS: 0x{}\nV: 0x{}\nSigner address: 0x{}",
        hex::encode(r),
        hex::encode(s),
        hex::encode(v),
        from_address_hex,
    );
    let rs = Signature::try_from(&signature[0..64])?;
    let v = RecoveryId::try_from(v[0])?;
    hasher.update(hash);
    let r_key = VerifyingKey::recover_from_digest(hasher, &rs, v)?;
    let r_key_addr = get_eth_address_bytes(&r_key);
    let r_key_addr = r_key_addr.as_ref();
    ensure!(
        from_address == r_key_addr.as_ref(),
        "Signature check failed! expected_signer=0x{} actual_signer=0x{}",
        from_address_hex,
        hex::encode(r_key_addr)
    );
    debug!(
        "Signer public key: 0x{}",
        hex::encode(r_key.to_sec1_bytes())
    );
    debug!(
        "Last touched: 0x{}",
        if let Some(addr) = &msg.last_edge_addr {
            let addr = addr.as_slice();
            hex::encode(addr)
        } else {
            "None".to_string()
        }
    );

    Ok((msg, raw_msg))
}
