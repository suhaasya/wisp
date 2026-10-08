//! In-app navigation targets for the shell router.

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Route {
    #[default]
    Connections,
    ConnectionForm,
    Workspace,
    #[cfg(feature = "ui-gallery")]
    Gallery,
}

impl Route {
    pub fn title(self) -> &'static str {
        match self {
            Self::Connections => "Connections",
            Self::ConnectionForm => "New connection",
            Self::Workspace => "Workspace",
            #[cfg(feature = "ui-gallery")]
            Self::Gallery => "Component gallery",
        }
    }
}
