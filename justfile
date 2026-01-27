set dotenv-load := true

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
    @rm -rf ./db/*sqlite*
