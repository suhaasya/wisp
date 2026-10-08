//! Secret kinds keyed with a connection id.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretKind {
    Password,
    SshPassphrase,
    Token,
}

impl SecretKind {
    pub const ALL: [SecretKind; 3] = [
        SecretKind::Password,
        SecretKind::SshPassphrase,
        SecretKind::Token,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Password => "password",
            Self::SshPassphrase => "ssh_passphrase",
            Self::Token => "token",
        }
    }
}

pub fn storage_key(connection_id: &str, kind: SecretKind) -> String {
    format!("connection/{connection_id}/{}", kind.as_str())
}
