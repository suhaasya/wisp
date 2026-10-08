//! LUM-014: saved connections persistence and store behaviour.

use std::collections::BTreeMap;

use proptest::{collection::btree_map, option, prelude::*};
use tempfile::TempDir;
use wisp_store::{
    parse_connections_toml, ConnectionEngine, ConnectionFolder, ConnectionId, ConnectionProfile,
    ConnectionStore, ConnectionsFile, ConnectionsLoadError, EnvironmentTag, MockSecretStore,
    Secret, SecretKind, SecretStore, SslMode, SslSettings, SshSettings, TransportKind,
    CONNECTIONS_VERSION,
};

fn temp_store() -> (TempDir, ConnectionStore) {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("connections.toml");
    (dir, ConnectionStore::empty_at(path))
}

#[test]
fn profile_round_trips_through_toml_unchanged() {
    let folder_id = ConnectionId::new_v7();
    let profile_id = ConnectionId::new_v7();
    let file = ConnectionsFile {
        version: CONNECTIONS_VERSION,
        folders: vec![ConnectionFolder {
            id: folder_id,
            name: "Prod".into(),
            order: 0,
        }],
        profiles: vec![ConnectionProfile {
            id: profile_id,
            name: "analytics".into(),
            folder_id: Some(folder_id),
            engine: ConnectionEngine::PostgreSql,
            transport: TransportKind::Tcp,
            host: Some("db.example.com".into()),
            port: Some(5432),
            user: Some("wisp".into()),
            database: Some("app".into()),
            ssl: SslSettings {
                mode: SslMode::Require,
            },
            ssh: SshSettings {
                enabled: true,
                host: Some("jump.example.com".into()),
                port: Some(22),
                user: Some("ubuntu".into()),
            },
            options: BTreeMap::from([("application_name".into(), "wisp".into())]),
            env_tag: EnvironmentTag::Production,
            read_only: true,
            safe_mode: true,
            colour: Some("#336699".into()),
            last_used_unix: Some(1_700_000_000),
        }],
    };

    let text = toml::to_string_pretty(&file).expect("serialize");
    let parsed = parse_connections_toml(&text).expect("parse");
    assert_eq!(parsed, file);

    let text2 = toml::to_string_pretty(&parsed).expect("reserialize");
    assert_eq!(text2, text, "stable pretty TOML on second write");
}

#[test]
fn store_persists_and_reloads() {
    let (_dir, mut store) = temp_store();
    let profile = ConnectionProfile::new("local", ConnectionEngine::MySql);
    let id = profile.id;
    store.create(profile).expect("create");
    store.save().expect("save");

    let loaded = ConnectionStore::load_from_path(store.path().to_path_buf()).expect("load");
    assert_eq!(loaded.get(id).map(|p| p.name.as_str()), Some("local"));
}

#[test]
fn corrupt_file_is_backed_up_with_user_message() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("connections.toml");
    std::fs::write(&path, "version = not-a-number\n").expect("write corrupt");

    let err = ConnectionStore::load_from_path(path.clone()).expect_err("corrupt load");
    let ConnectionsLoadError::Corrupt {
        message,
        backup_path,
        ..
    } = err
    else {
        panic!("expected Corrupt error");
    };
    assert!(message.contains("backup was created"));
    assert!(backup_path.is_file());
    assert!(path.is_file(), "original file left in place for user repair");
}

#[test]
fn duplicate_copies_secrets_under_new_id() {
    let (_dir, mut store) = temp_store();
    let profile = ConnectionProfile::new("primary", ConnectionEngine::PostgreSql);
    let id = profile.id;
    store.create(profile).expect("create");

    let secrets = MockSecretStore::new();
    secrets
        .set(
            &id.to_string(),
            SecretKind::Password,
            &Secret::from_utf8("s3cret"),
        )
        .expect("set password");
    secrets
        .set(
            &id.to_string(),
            SecretKind::Token,
            &Secret::from_utf8("tok"),
        )
        .expect("set token");

    let new_id = store.duplicate(id, &secrets).expect("duplicate");
    assert_ne!(new_id, id);
    assert_eq!(
        secrets
            .get(&new_id.to_string(), SecretKind::Password)
            .expect("copy password")
            .expose_str(),
        "s3cret"
    );
    assert_eq!(
        secrets
            .get(&new_id.to_string(), SecretKind::Token)
            .expect("copy token")
            .expose_str(),
        "tok"
    );
}

#[test]
fn folder_ordering_and_move() {
    let (_dir, mut store) = temp_store();
    let a = store.create_folder("A");
    let b = store.create_folder("B");
    store.reorder_folders(&[b, a]).expect("reorder");

    let order: Vec<_> = store
        .folders_sorted()
        .iter()
        .map(|f| f.id)
        .collect();
    assert_eq!(order, vec![b, a]);

    let mut profile = ConnectionProfile::new("x", ConnectionEngine::MariaDb);
    profile.folder_id = Some(b);
    let pid = profile.id;
    store.create(profile).expect("create");
    store.move_to_folder(pid, Some(a)).expect("move");
    assert_eq!(store.get(pid).and_then(|p| p.folder_id), Some(a));
}

#[test]
fn hundred_profiles_under_100kb_on_disk() {
    let dir = TempDir::new().expect("tempdir");
    let path = dir.path().join("connections.toml");
    let mut store = ConnectionStore::empty_at(path.clone());
    for i in 0..100 {
        let mut p = ConnectionProfile::new(format!("conn-{i}"), ConnectionEngine::PostgreSql);
        p.host = Some("127.0.0.1".into());
        p.port = Some(5432);
        p.user = Some("user".into());
        p.database = Some("db".into());
        store.create(p).expect("create");
    }
    let bytes = std::fs::metadata(&path).expect("metadata").len();
    assert!(
        bytes < 100 * 1024,
        "connections.toml size {bytes} bytes, budget is 100 KiB"
    );
}

fn arb_env_tag() -> impl Strategy<Value = EnvironmentTag> {
    prop_oneof![
        Just(EnvironmentTag::Development),
        Just(EnvironmentTag::Staging),
        Just(EnvironmentTag::Production),
        "[a-z]{1,12}".prop_map(EnvironmentTag::Custom),
    ]
}

fn arb_profile() -> impl Strategy<Value = ConnectionProfile> {
    (
        btree_map(any::<String>(), any::<String>(), 0..4),
        any::<bool>(),
        any::<bool>(),
        option::of(any::<i64>()),
        option::of("#[0-9a-fA-F]{6}".prop_map(|s| format!("#{s}"))),
        arb_env_tag(),
        prop_oneof![
            Just(ConnectionEngine::PostgreSql),
            Just(ConnectionEngine::MySql),
            Just(ConnectionEngine::MariaDb),
        ],
        prop_oneof![
            Just(TransportKind::Tcp),
            Just(TransportKind::UnixSocket),
            Just(TransportKind::NamedPipe),
        ],
        prop_oneof![
            Just(SslMode::Disable),
            Just(SslMode::Prefer),
            Just(SslMode::Require),
            Just(SslMode::VerifyCa),
            Just(SslMode::VerifyFull),
        ],
    )
        .prop_map(
            |(options, read_only, safe_mode, last_used_unix, colour, env_tag, engine, transport, ssl_mode)| {
                ConnectionProfile {
                    id: ConnectionId::new_v7(),
                    name: "prop-test".into(),
                    folder_id: None,
                    engine,
                    transport,
                    host: Some("localhost".into()),
                    port: Some(5432),
                    user: Some("u".into()),
                    database: Some("d".into()),
                    ssl: SslSettings { mode: ssl_mode },
                    ssh: SshSettings::default(),
                    options,
                    env_tag,
                    read_only,
                    safe_mode,
                    colour,
                    last_used_unix,
                }
            },
        )
}

fn arb_connections_file() -> impl Strategy<Value = ConnectionsFile> {
    prop::collection::vec(arb_profile(), 0..8).prop_map(|profiles| ConnectionsFile {
        version: CONNECTIONS_VERSION,
        folders: Vec::new(),
        profiles,
    })
}

proptest! {
    #[test]
    fn random_profiles_serialize_deserialize_equal(file in arb_connections_file()) {
        let text = toml::to_string_pretty(&file).expect("serialize");
        let parsed = parse_connections_toml(&text).expect("parse");
        prop_assert_eq!(parsed, file);
    }
}
