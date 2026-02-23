set dotenv-load := true
export RUST_LOG := "info"

default:
    @just --list

alias fix := lint-fix

# cargo new lib crate
new project_name:
    @mkdir -p ./crates
    @cargo new ./crates/{{ project_name }} --lib --vcs none

lint-fix:
    @cargo x lint --fix

[confirm("Are you sure you want to prune the database? This will delete all data in the database.")]
prune-db:
    @rm -rf ./data/*sqlite*

sqlx-prepare:
    @cargo sqlx prepare --database-url ${DATABASE_URL} --workspace

watch_hot_video:
    @cargo run watch_hot_video --watch_id $WATCH_ID --douban_user_id $DOUBAN_USER_ID

watch_hot_and_persistent:
    @cargo run watch_hot_and_persistent

dist:
    @cargo x dist --package emos --strip true
