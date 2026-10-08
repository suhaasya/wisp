//! In-memory secret buffer (zeroized on drop).

use std::{borrow::Borrow, fmt, ops::Deref, str};

use zeroize::{Zeroize, Zeroizing};

/// UTF-8 secret material cleared from memory when dropped.
#[derive(Clone, Zeroize)]
#[zeroize(drop)]
pub struct Secret(Zeroizing<Vec<u8>>);

impl Secret {
    pub fn new(value: impl AsRef<[u8]>) -> Self {
        Self(Zeroizing::new(value.as_ref().to_vec()))
    }

    pub fn from_utf8(value: &str) -> Self {
        Self::new(value.as_bytes())
    }

    pub fn expose_str(&self) -> &str {
        str::from_utf8(&self.0).unwrap_or("")
    }

    pub fn expose_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret([redacted])")
    }
}

impl PartialEq for Secret {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_slice() == other.0.as_slice()
    }
}

impl Eq for Secret {}

impl From<String> for Secret {
    fn from(value: String) -> Self {
        Self(Zeroizing::new(value.into_bytes()))
    }
}

impl Borrow<[u8]> for Secret {
    fn borrow(&self) -> &[u8] {
        &self.0
    }
}

impl Deref for Secret {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
