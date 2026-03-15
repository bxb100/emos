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
    @cargo run watch_hot_video --douban_user_id $DOUBAN_USER_ID

watch_hot_and_persistent:
    @cargo run watch_hot_and_persistent

tmdb_scifi_media:
    @cargo run tmdb_scifi_media --flag

tmdb_download_cover:
    @echo '{{ style("warning") }}This receipt build for test{{ NORMAL }}'
    @cargo run tmdb_download_cover --video --id 1389149 --id 991494 --id 4247 --id 1319280 --id 1233413 --namespace cs
    @cargo run tmdb_download_cover --id 1408 --id 59941 --id 65733 --id 1399 --id 2734 --namespace cs

gen namespace zh en:
    @just --justfile ./lib/cover_generator/Justfile gen {{namespace}} {{zh}} {{en}}

dist:
    @cargo x dist --package emos --strip true
