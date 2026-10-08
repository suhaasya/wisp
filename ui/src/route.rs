//! In-app navigation targets for the shell router.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Route {
    #[default]
    Connections,
    ConnectionForm,
    Workspace,
}

impl Route {
    pub fn title(self) -> &'static str {
        match self {
            Self::Connections => "Connections",
            Self::ConnectionForm => "New connection",
            Self::Workspace => "Workspace",
        }
    }
}
