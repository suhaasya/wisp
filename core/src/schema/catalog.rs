//! Database object catalog for the schema sidebar (names + ids only).

use std::collections::BTreeMap;

use wisp_store::{ConnectionEngine, ConnectionProfile};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SchemaObjectId(pub u32);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SchemaObjectKind {
    Tables,
    Views,
    Functions,
    Sequences,
    Types,
}

impl SchemaObjectKind {
    pub const ALL: [Self; 5] = [
        Self::Tables,
        Self::Views,
        Self::Functions,
        Self::Sequences,
        Self::Types,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::Tables => "Tables",
            Self::Views => "Views",
            Self::Functions => "Functions",
            Self::Sequences => "Sequences",
            Self::Types => "Types",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaObject {
    pub id: SchemaObjectId,
    pub schema: String,
    pub name: String,
    pub kind: SchemaObjectKind,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SchemaCatalog {
    pub connection_label: String,
    pub database: String,
    pub schemas: Vec<String>,
    pub objects: Vec<SchemaObject>,
}

/// Sidebar before any connection is selected.
pub fn catalog_disconnected() -> SchemaCatalog {
    SchemaCatalog {
        connection_label: "Not connected".into(),
        database: String::new(),
        schemas: Vec::new(),
        objects: Vec::new(),
    }
}

/// Empty catalog while introspection runs (uses saved profile database/name).
pub fn catalog_placeholder(profile: &ConnectionProfile) -> SchemaCatalog {
    let database = profile.database.clone().unwrap_or_default();
    let schemas = match profile.engine {
        ConnectionEngine::PostgreSql => vec!["public".into()],
        ConnectionEngine::MySql | ConnectionEngine::MariaDb => {
            if database.is_empty() {
                vec![]
            } else {
                vec![database.clone()]
            }
        }
    };
    SchemaCatalog {
        connection_label: profile.name.clone(),
        database,
        schemas,
        objects: Vec::new(),
    }
}

impl SchemaCatalog {
    pub fn objects_in_schema<'a>(
        &'a self,
        schema: &'a str,
    ) -> impl Iterator<Item = &'a SchemaObject> + 'a {
        self.objects
            .iter()
            .filter(move |o| o.schema == schema)
    }

    pub fn count_by_kind(&self, schema: &str) -> BTreeMap<SchemaObjectKind, usize> {
        let mut counts = BTreeMap::new();
        for kind in SchemaObjectKind::ALL {
            counts.insert(kind, 0);
        }
        for obj in self.objects_in_schema(schema) {
            *counts.entry(obj.kind).or_insert(0) += 1;
        }
        counts
    }
}
