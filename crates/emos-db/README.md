# emos-db

SQLite storage for the local video catalog. Import the public API with
`use emos_db::{Video, VideoRepository}`.

`VideoRepository::open().await` opens the workspace's `data/emos.sqlite` and
applies the migrations in this crate. These paths are configured at build time.

The repository provides:

- `find_by_genre_after`: a page of videos filtered by genre, ordered by `todb_id`.
- `insert_videos`: inserts `emos_client::video::Video` values and returns the
  number of affected rows.
- `existing_todb_ids`: finds which supplied IDs already exist; input must be
  nonempty.
- `find_by_title_prefix`: matches a trimmed title prefix using SQLite `LIKE`
  semantics, including its wildcard handling.

The private `repository` module owns the connection and queries; `video` defines
the stored row type. SQL and the database schema retain their existing format.

The current database tests use the workspace database. `cargo test -p emos-db --no-run` compiles them without opening or migrating it at runtime.
