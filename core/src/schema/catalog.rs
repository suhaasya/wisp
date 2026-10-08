//! Database object catalog for the schema sidebar (names + ids only).

use std::collections::BTreeMap;

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

#[derive(Debug, Clone, Default)]
pub struct SchemaCatalog {
    pub connection_label: String,
    pub database: String,
    pub schemas: Vec<String>,
    pub objects: Vec<SchemaObject>,
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
