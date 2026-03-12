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
    @cargo run tmdb_scifi_media

tmdb_download_cover:
    @echo '{{ style("warning") }}This receipt build for test{{ NORMAL }}'
    @cargo run tmdb_download_cover --video --id 1389149 --id 991494 --id 4247 --id 1319280 --id 1233413 --id 1305781 --id 798645 --id 911430 --id 1499984 --id 1381027 --namespace cs

generate_cover: tmdb_download_cover
    @echo '{{ style("warning") }}This receipt build for test{{ NORMAL }}'
    @just --justfile ./lib/cover_generator/Justfile gen cs "测试" "test"

dist:
    @cargo x dist --package emos --strip true
