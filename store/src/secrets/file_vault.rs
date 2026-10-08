//! Encrypted on-disk vault (headless Linux fallback). Lives under **data**, not config.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use argon2::{Argon2, Params, Version};
use chacha20poly1305::{aead::{Aead, KeyInit}, XChaCha20Poly1305, XNonce};
use rand::{rngs::OsRng, RngCore};
use serde::{Deserialize, Serialize};
use zeroize::Zeroizing;

use super::{
    error::SecretError,
    kind::{storage_key, SecretKind},
    secret::Secret,
    store::SecretStore,
};

const MAGIC: &[u8; 8] = b"WISPSEC1";
const SALT_LEN: usize = 16;
const KEY_LEN: usize = 32;
const NONCE_LEN: usize = 24;

#[derive(Serialize, Deserialize, Default)]
struct VaultPlaintext {
    entries: HashMap<String, String>,
}

pub struct FileVaultStore {
    path: PathBuf,
    salt: [u8; SALT_LEN],
    key: Zeroizing<[u8; KEY_LEN]>,
    state: Mutex<VaultPlaintext>,
}

impl FileVaultStore {
    pub fn vault_path(paths: &crate::paths::WispPaths) -> PathBuf {
        paths.data_dir().join("secrets.vault")
    }

    pub fn create(path: PathBuf, master: &Secret) -> Result<Self, SecretError> {
        if path.exists() {
            return Err(SecretError::Backend("vault already exists".into()));
        }
        let mut salt = [0u8; SALT_LEN];
        OsRng.fill_bytes(&mut salt);
        let key = derive_key(master, &salt)?;
        let store = Self {
            path,
            salt,
            key,
            state: Mutex::new(VaultPlaintext::default()),
        };
        store.persist()?;
        Ok(store)
    }

    pub fn unlock(path: PathBuf, master: &Secret) -> Result<Self, SecretError> {
        let (salt, plaintext) = decrypt_file(&path, master)?;
        let key = derive_key(master, &salt)?;
        Ok(Self {
            path,
            salt,
            key,
            state: Mutex::new(plaintext),
        })
    }

    fn persist(&self) -> Result<(), SecretError> {
        let state = self.state.lock().expect("vault lock");
        encrypt_file(&self.path, &self.salt, &self.key, &state)
    }
}

impl SecretStore for FileVaultStore {
    fn get(&self, connection_id: &str, kind: SecretKind) -> Result<Secret, SecretError> {
        let key = storage_key(connection_id, kind);
        let state = self.state.lock().expect("vault lock");
        state
            .entries
            .get(&key)
            .map(|v| Secret::from(v.clone()))
            .ok_or(SecretError::NotFound)
    }

    fn set(&self, connection_id: &str, kind: SecretKind, value: &Secret) -> Result<(), SecretError> {
        {
            let mut state = self.state.lock().expect("vault lock");
            state
                .entries
                .insert(storage_key(connection_id, kind), value.expose_str().to_string());
        }
        self.persist()
    }

    fn delete(&self, connection_id: &str, kind: SecretKind) -> Result<(), SecretError> {
        {
            let mut state = self.state.lock().expect("vault lock");
            state.entries.remove(&storage_key(connection_id, kind));
        }
        self.persist()
    }
}

fn derive_key(master: &Secret, salt: &[u8]) -> Result<Zeroizing<[u8; KEY_LEN]>, SecretError> {
    let params =
        Params::new(19 * 1024, 2, 1, Some(KEY_LEN)).map_err(|e| SecretError::Backend(e.to_string()))?;
    let argon2 = Argon2::new(argon2::Algorithm::Argon2id, Version::V0x13, params);
    let mut key = Zeroizing::new([0u8; KEY_LEN]);
    argon2
        .hash_password_into(master.expose_bytes(), salt, key.as_mut())
        .map_err(|e| SecretError::Backend(e.to_string()))?;
    Ok(key)
}

fn encrypt_file(
    path: &Path,
    salt: &[u8; SALT_LEN],
    key: &Zeroizing<[u8; KEY_LEN]>,
    payload: &VaultPlaintext,
) -> Result<(), SecretError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| SecretError::Backend(e.to_string()))?;
    }
    let json = serde_json::to_vec(payload).map_err(|e| SecretError::Backend(e.to_string()))?;
    let cipher =
        XChaCha20Poly1305::new_from_slice(key.as_slice()).map_err(|e| SecretError::Backend(e.to_string()))?;
    let mut nonce = [0u8; NONCE_LEN];
    OsRng.fill_bytes(&mut nonce);
    let ciphertext = cipher
        .encrypt(XNonce::from_slice(&nonce), json.as_ref())
        .map_err(|e| SecretError::Backend(e.to_string()))?;
    let mut file = Vec::with_capacity(MAGIC.len() + SALT_LEN + NONCE_LEN + ciphertext.len());
    file.extend_from_slice(MAGIC);
    file.extend_from_slice(salt);
    file.extend_from_slice(&nonce);
    file.extend_from_slice(&ciphertext);
    let tmp = path.with_extension("vault.tmp");
    fs::write(&tmp, &file).map_err(|e| SecretError::Backend(e.to_string()))?;
    fs::rename(tmp, path).map_err(|e| SecretError::Backend(e.to_string()))?;
    Ok(())
}

fn decrypt_file(path: &Path, master: &Secret) -> Result<([u8; SALT_LEN], VaultPlaintext), SecretError> {
    let file = fs::read(path).map_err(|e| SecretError::Backend(e.to_string()))?;
    if file.len() < MAGIC.len() + SALT_LEN + NONCE_LEN {
        return Err(SecretError::CorruptVault);
    }
    if &file[..MAGIC.len()] != MAGIC {
        return Err(SecretError::CorruptVault);
    }
    let offset = MAGIC.len();
    let mut salt = [0u8; SALT_LEN];
    salt.copy_from_slice(&file[offset..offset + SALT_LEN]);
    let nonce = &file[offset + SALT_LEN..offset + SALT_LEN + NONCE_LEN];
    let ciphertext = &file[offset + SALT_LEN + NONCE_LEN..];
    let key = derive_key(master, &salt)?;
    let cipher =
        XChaCha20Poly1305::new_from_slice(key.as_slice()).map_err(|e| SecretError::Backend(e.to_string()))?;
    let plain = cipher
        .decrypt(XNonce::from_slice(nonce), ciphertext)
        .map_err(|_| SecretError::BadMasterPassword)?;
    let payload: VaultPlaintext =
        serde_json::from_slice(&plain).map_err(|_| SecretError::CorruptVault)?;
    Ok((salt, payload))
}
