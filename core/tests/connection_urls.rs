//! Table-driven connection URL samples (LUM-017).

use wisp_core::{
    parse_connection_paste, ConnectionFormDraft, FormEngine, UrlParseError,
};
use wisp_store::SslMode;

#[derive(Clone, Copy)]
struct Expect {
    engine: FormEngine,
    host: &'static str,
    port: Option<&'static str>,
    user: Option<&'static str>,
    password: Option<&'static str>,
    database: Option<&'static str>,
    ssl: Option<SslMode>,
    option: Option<(&'static str, &'static str)>,
}

fn check(input: &str, exp: Expect) {
    let d = parse_connection_paste(input).unwrap_or_else(|e| panic!("{input:?}: {e}"));
    assert_eq!(d.engine, exp.engine, "{input}");
    assert_eq!(d.host, exp.host, "{input}");
    if let Some(port) = exp.port {
        assert_eq!(d.port, port, "{input}");
    }
    if let Some(user) = exp.user {
        assert_eq!(d.user, user, "{input}");
    }
    if let Some(password) = exp.password {
        assert_eq!(d.password, password, "{input}");
    }
    if let Some(db) = exp.database {
        assert_eq!(d.database, db, "{input}");
    }
    if let Some(ssl) = exp.ssl {
        assert_eq!(d.ssl_mode, ssl, "{input}");
    }
    if let Some((k, v)) = exp.option {
        assert_eq!(d.extra_options.get(k).map(String::as_str), Some(v), "{input}");
    }
}

const PG: FormEngine = FormEngine::PostgreSql;
const MY: FormEngine = FormEngine::MySql;
const MA: FormEngine = FormEngine::MariaDb;

#[test]
fn real_world_connection_url_table() {
    const SAMPLES: &[(&str, Expect)] = &[
        (
            "postgres://app:secret@db.example.com:5432/shop",
            Expect {
                engine: PG,
                host: "db.example.com",
                port: Some("5432"),
                user: Some("app"),
                password: Some("secret"),
                database: Some("shop"),
                ssl: None,
                option: None,
            },
        ),
        (
            "postgresql://u@localhost/mydb",
            Expect {
                engine: PG,
                host: "localhost",
                port: Some("5432"),
                user: Some("u"),
                password: Some(""),
                database: Some("mydb"),
                ssl: None,
                option: None,
            },
        ),
        (
            "postgres://u:p%40ss%2Fword@h/db",
            Expect {
                engine: PG,
                host: "h",
                port: None,
                user: Some("u"),
                password: Some("p@ss/word"),
                database: Some("db"),
                ssl: None,
                option: None,
            },
        ),
        (
            "postgres://u@h/db?sslmode=require",
            Expect {
                engine: PG,
                host: "h",
                port: None,
                user: Some("u"),
                password: None,
                database: Some("db"),
                ssl: Some(SslMode::Require),
                option: None,
            },
        ),
        (
            "postgres://u@h/db?sslmode=verify-full",
            Expect {
                engine: PG,
                host: "h",
                port: None,
                user: None,
                password: None,
                database: Some("db"),
                ssl: Some(SslMode::VerifyFull),
                option: None,
            },
        ),
        (
            "postgres://u@h/db?sslmode=prefer",
            Expect {
                engine: PG,
                host: "h",
                port: None,
                user: None,
                password: None,
                database: Some("db"),
                ssl: Some(SslMode::Prefer),
                option: None,
            },
        ),
        (
            "postgres://u@h/db?sslmode=disable",
            Expect {
                engine: PG,
                host: "h",
                port: None,
                user: None,
                password: None,
                database: Some("db"),
                ssl: Some(SslMode::Disable),
                option: None,
            },
        ),
        (
            "postgres://u@h/db?application_name=wisp&foo=bar",
            Expect {
                engine: PG,
                host: "h",
                port: None,
                user: Some("u"),
                password: None,
                database: Some("db"),
                ssl: None,
                option: Some(("application_name", "wisp")),
            },
        ),
        (
            "postgres://u@h/db?options=-c%20search_path%3Dapp",
            Expect {
                engine: PG,
                host: "h",
                port: None,
                user: None,
                password: None,
                database: Some("db"),
                ssl: None,
                option: Some(("options", "-c search_path=app")),
            },
        ),
        (
            "postgres://u@ep-cool-name.neon.tech/neondb",
            Expect {
                engine: PG,
                host: "ep-cool-name.neon.tech",
                port: None,
                user: Some("u"),
                password: None,
                database: Some("neondb"),
                ssl: Some(SslMode::Require),
                option: None,
            },
        ),
        (
            "postgres://postgres:pw@db.abcdef.supabase.co:5432/postgres",
            Expect {
                engine: PG,
                host: "db.abcdef.supabase.co",
                port: Some("5432"),
                user: Some("postgres"),
                password: Some("pw"),
                database: Some("postgres"),
                ssl: Some(SslMode::Require),
                option: None,
            },
        ),
        (
            "postgres://admin:pw@mydb.abc123.us-east-1.rds.amazonaws.com:5432/production",
            Expect {
                engine: PG,
                host: "mydb.abc123.us-east-1.rds.amazonaws.com",
                port: Some("5432"),
                user: Some("admin"),
                password: Some("pw"),
                database: Some("production"),
                ssl: Some(SslMode::Require),
                option: None,
            },
        ),
        (
            "postgres://u@127.0.0.1:5433/work",
            Expect {
                engine: PG,
                host: "127.0.0.1",
                port: Some("5433"),
                user: Some("u"),
                password: Some(""),
                database: Some("work"),
                ssl: None,
                option: None,
            },
        ),
        (
            "postgres://u@h/db?target_session_attrs=read-write",
            Expect {
                engine: PG,
                host: "h",
                port: None,
                user: Some("u"),
                password: None,
                database: Some("db"),
                ssl: None,
                option: Some(("target_session_attrs", "read-write")),
            },
        ),
        (
            "mysql://root:pw@127.0.0.1:3307/app?ssl-mode=REQUIRED&charset=utf8mb4",
            Expect {
                engine: MY,
                host: "127.0.0.1",
                port: Some("3307"),
                user: Some("root"),
                password: Some("pw"),
                database: Some("app"),
                ssl: Some(SslMode::Require),
                option: Some(("charset", "utf8mb4")),
            },
        ),
        (
            "mysql://u@host/db?ssl-mode=PREFERRED",
            Expect {
                engine: MY,
                host: "host",
                port: Some("3306"),
                user: Some("u"),
                password: Some(""),
                database: Some("db"),
                ssl: Some(SslMode::Prefer),
                option: None,
            },
        ),
        (
            "mysql://u@aws.connect.psdb.cloud/main",
            Expect {
                engine: MY,
                host: "aws.connect.psdb.cloud",
                port: None,
                user: Some("u"),
                password: None,
                database: Some("main"),
                ssl: Some(SslMode::Require),
                option: None,
            },
        ),
        (
            "mysql://u@primary.planetscale.com/app",
            Expect {
                engine: MY,
                host: "primary.planetscale.com",
                port: None,
                user: Some("u"),
                password: None,
                database: Some("app"),
                ssl: Some(SslMode::Require),
                option: None,
            },
        ),
        (
            "mariadb://u@host/db",
            Expect {
                engine: MA,
                host: "host",
                port: Some("3306"),
                user: Some("u"),
                password: Some(""),
                database: Some("db"),
                ssl: None,
                option: None,
            },
        ),
        (
            "mariadb://root@10.0.0.5:3306/mysql",
            Expect {
                engine: MA,
                host: "10.0.0.5",
                port: Some("3306"),
                user: Some("root"),
                password: Some(""),
                database: Some("mysql"),
                ssl: None,
                option: None,
            },
        ),
        (
            "host=localhost port=5433 dbname=shop user=app password=x sslmode=verify-full",
            Expect {
                engine: PG,
                host: "localhost",
                port: Some("5433"),
                user: Some("app"),
                password: Some("x"),
                database: Some("shop"),
                ssl: Some(SslMode::VerifyFull),
                option: None,
            },
        ),
        (
            "host='db.example.com' user='app' password='s ec ret' dbname=analytics sslmode=require",
            Expect {
                engine: PG,
                host: "db.example.com",
                port: Some("5432"),
                user: Some("app"),
                password: Some("s ec ret"),
                database: Some("analytics"),
                ssl: Some(SslMode::Require),
                option: None,
            },
        ),
        (
            "DATABASE_URL=\"postgres://u:pw@h/db\"",
            Expect {
                engine: PG,
                host: "h",
                port: None,
                user: Some("u"),
                password: Some("pw"),
                database: Some("db"),
                ssl: None,
                option: None,
            },
        ),
        (
            "export DATABASE_URL=postgres://u@h/db",
            Expect {
                engine: PG,
                host: "h",
                port: None,
                user: Some("u"),
                password: None,
                database: Some("db"),
                ssl: None,
                option: None,
            },
        ),
        (
            "STAGING_DATABASE_URL=postgresql://ci:ci@pg.internal:5432/ci",
            Expect {
                engine: PG,
                host: "pg.internal",
                port: Some("5432"),
                user: Some("ci"),
                password: Some("ci"),
                database: Some("ci"),
                ssl: None,
                option: None,
            },
        ),
        (
            "DB_URL=mysql://app:pass@db.host/app",
            Expect {
                engine: MY,
                host: "db.host",
                port: None,
                user: Some("app"),
                password: Some("pass"),
                database: Some("app"),
                ssl: None,
                option: None,
            },
        ),
        (
            "postgres://u:complex%23hash@h/db",
            Expect {
                engine: PG,
                host: "h",
                port: None,
                user: Some("u"),
                password: Some("complex#hash"),
                database: Some("db"),
                ssl: None,
                option: None,
            },
        ),
        (
            "postgres://u@h:6543/db?sslmode=require&connect_timeout=10",
            Expect {
                engine: PG,
                host: "h",
                port: Some("6543"),
                user: Some("u"),
                password: None,
                database: Some("db"),
                ssl: Some(SslMode::Require),
                option: Some(("connect_timeout", "10")),
            },
        ),
        (
            "postgres://u@h/db?sslmode=verify-ca",
            Expect {
                engine: PG,
                host: "h",
                port: None,
                user: None,
                password: None,
                database: Some("db"),
                ssl: Some(SslMode::VerifyCa),
                option: None,
            },
        ),
        (
            "mysql://u@h/db?sslmode=REQUIRED",
            Expect {
                engine: MY,
                host: "h",
                port: None,
                user: Some("u"),
                password: None,
                database: Some("db"),
                ssl: Some(SslMode::Require),
                option: None,
            },
        ),
        (
            "mysql://u@h/db?ssl-mode=VERIFY_IDENTITY",
            Expect {
                engine: MY,
                host: "h",
                port: None,
                user: None,
                password: None,
                database: Some("db"),
                ssl: Some(SslMode::VerifyFull),
                option: None,
            },
        ),
        (
            "postgres://u@h/db?keepalives=1&keepalives_idle=30",
            Expect {
                engine: PG,
                host: "h",
                port: None,
                user: None,
                password: None,
                database: Some("db"),
                ssl: None,
                option: Some(("keepalives", "1")),
            },
        ),
        (
            "host=127.0.0.1 dbname=postgres user=postgres application_name=cli",
            Expect {
                engine: PG,
                host: "127.0.0.1",
                port: Some("5432"),
                user: Some("postgres"),
                password: Some(""),
                database: Some("postgres"),
                ssl: None,
                option: Some(("application_name", "cli")),
            },
        ),
        (
            "postgres://u@instance.sql.cloud.google.com/mydb",
            Expect {
                engine: PG,
                host: "instance.sql.cloud.google.com",
                port: None,
                user: Some("u"),
                password: None,
                database: Some("mydb"),
                ssl: Some(SslMode::Require),
                option: None,
            },
        ),
        (
            "postgres://u@10.1.2.3/cloudsql",
            Expect {
                engine: PG,
                host: "10.1.2.3",
                port: None,
                user: Some("u"),
                password: None,
                database: Some("cloudsql"),
                ssl: None,
                option: None,
            },
        ),
        (
            "postgresql://:@localhost/postgres",
            Expect {
                engine: PG,
                host: "localhost",
                port: Some("5432"),
                user: Some(""),
                password: Some(""),
                database: Some("postgres"),
                ssl: None,
                option: None,
            },
        ),
        (
            "postgres://user%40domain@host/db",
            Expect {
                engine: PG,
                host: "host",
                port: None,
                user: Some("user@domain"),
                password: Some(""),
                database: Some("db"),
                ssl: None,
                option: None,
            },
        ),
        (
            "mysql://u:pw@host/db?charset=latin1&timezone=UTC",
            Expect {
                engine: MY,
                host: "host",
                port: None,
                user: Some("u"),
                password: Some("pw"),
                database: Some("db"),
                ssl: None,
                option: Some(("charset", "latin1")),
            },
        ),
        (
            "postgres://u@h/",
            Expect {
                engine: PG,
                host: "h",
                port: None,
                user: Some("u"),
                password: None,
                database: Some("postgres"),
                ssl: None,
                option: None,
            },
        ),
        (
            "mysql://u@h/",
            Expect {
                engine: MY,
                host: "h",
                port: None,
                user: Some("u"),
                password: None,
                database: Some("mysql"),
                ssl: None,
                option: None,
            },
        ),
        (
            "  postgres://u@h/db  ",
            Expect {
                engine: PG,
                host: "h",
                port: None,
                user: Some("u"),
                password: None,
                database: Some("db"),
                ssl: None,
                option: None,
            },
        ),
        (
            "host=neon.host dbname=main user=u password=p sslmode=prefer",
            Expect {
                engine: PG,
                host: "neon.host",
                port: Some("5432"),
                user: Some("u"),
                password: Some("p"),
                database: Some("main"),
                ssl: Some(SslMode::Prefer),
                option: None,
            },
        ),
    ];

    assert!(SAMPLES.len() >= 40, "need 40+ samples, got {}", SAMPLES.len());
    for (input, exp) in SAMPLES {
        check(input, *exp);
    }
}

#[test]
fn rejects_unrecognized_paste() {
    assert!(matches!(
        parse_connection_paste("not a url"),
        Err(UrlParseError::Unrecognized)
    ));
    assert!(matches!(
        parse_connection_paste(""),
        Err(UrlParseError::Unrecognized)
    ));
}

#[test]
fn debug_never_contains_password_from_table() {
    let d: ConnectionFormDraft =
        parse_connection_paste("postgres://u:supersecret@h/db").unwrap();
    let s = format!("{d:?}");
    assert!(!s.contains("supersecret"));
}
