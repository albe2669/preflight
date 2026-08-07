#![cfg(feature = "integration")]

//! Shared cursor behaviour exercised for both sync sources. The domain
//! crates previously duplicated these tests; here the source name is a
//! parameter so one harness covers every provider.

mod common;

use pretty_assertions::assert_eq;
use remote_sync::cursor;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use sync_state::entity::sync_state;

const SOURCES: [&str; 2] = ["github", "linear"];

#[tokio::test]
async fn test_cursor_put_then_get() {
    for source in SOURCES {
        let db = common::setup_db().await;

        // Put a cursor, then get it back
        cursor::put(&db, source, Some("abc123".into()), "ok", None)
            .await
            .unwrap();
        let got = cursor::get(&db, source).await.unwrap();
        assert_eq!(got, Some("abc123".into()));

        // Put with None cursor, get returns None
        cursor::put(&db, source, None, "ok", None).await.unwrap();
        let got = cursor::get(&db, source).await.unwrap();
        assert_eq!(got, None);
    }
}

#[tokio::test]
async fn test_cursor_put_updates_existing() {
    for source in SOURCES {
        let db = common::setup_db().await;

        cursor::put(&db, source, Some("first".into()), "ok", None)
            .await
            .unwrap();
        cursor::put(&db, source, Some("second".into()), "ok", None)
            .await
            .unwrap();

        let got = cursor::get(&db, source).await.unwrap();
        assert_eq!(got, Some("second".into()));
    }
}

#[tokio::test]
async fn test_cursor_get_returns_none_for_unknown_source() {
    for source in SOURCES {
        let db = common::setup_db().await;

        // The probe uses a source distinct from the two under test.
        let probe = format!("nonexistent-{source}");
        let got = cursor::get(&db, &probe).await.unwrap();
        assert_eq!(got, None);
    }
}

#[tokio::test]
async fn test_cursor_put_stores_status_and_error() {
    for source in SOURCES {
        let db = common::setup_db().await;

        cursor::put(
            &db,
            source,
            Some("page-5".into()),
            "ok",
            Some("some warning".into()),
        )
        .await
        .unwrap();

        let row = sync_state::Entity::find()
            .filter(sync_state::Column::Source.eq(source))
            .one(&db)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(row.cursor, Some("page-5".into()));
        assert_eq!(row.last_status, "ok");
        assert_eq!(row.last_error, Some("some warning".into()));
        assert!(row.last_synced_at.is_some());
    }
}
