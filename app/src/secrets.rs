//! Interactive secret store startup (keychain or encrypted vault).

use std::io::{self, IsTerminal};

use anyhow::{bail, Context};
use wisp_store::{
    secrets::FileVaultStore, BlockingSecretStore, OpenSecretStoreOptions, Secret, SecretError,
    WispPaths,
};

/// Open the OS keychain when available; otherwise prompt for a vault master password.
pub fn open_blocking_secret_store(paths: &WispPaths) -> anyhow::Result<BlockingSecretStore> {
    let vault_path = FileVaultStore::vault_path(paths);
    let mut master: Option<Secret> = None;

    loop {
        match wisp_store::open_secret_store(OpenSecretStoreOptions {
            paths,
            master_password: master.as_ref(),
        }) {
            Ok((store, _kind)) => return Ok(BlockingSecretStore::new(store)),
            Err(SecretError::MasterPasswordRequired) => {
                master = Some(prompt_master_password(vault_path.is_file())?);
            }
            Err(SecretError::BadMasterPassword) => {
                eprintln!("Incorrect master password.");
                master = None;
            }
            Err(err) => return Err(err.into()),
        }
    }
}

fn prompt_master_password(unlock_existing: bool) -> anyhow::Result<Secret> {
    if !io::stderr().is_terminal() {
        bail!(
            "no OS keychain available and encrypted vault requires a master password; \
             run Wisp from a terminal or set WISP_SECRET_BACKEND=mock for local dev"
        );
    }

    if unlock_existing {
        let value = read_password("Master password: ")?;
        return Ok(Secret::from_utf8(&value));
    }

    let first = read_password("Create a master password for stored connection secrets: ")?;
    let second = read_password("Confirm master password: ")?;
    if first != second {
        bail!("passwords did not match");
    }
    if first.is_empty() {
        bail!("master password cannot be empty");
    }
    Ok(Secret::from_utf8(&first))
}

fn read_password(prompt: &str) -> anyhow::Result<String> {
    rpassword::prompt_password(prompt).context("read master password")
}
