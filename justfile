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
