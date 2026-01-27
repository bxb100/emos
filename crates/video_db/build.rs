fn main() {
    let workspace_dir = env!("CARGO_WORKSPACE_DIR");
    let sqlite_database_url = format!("sqlite://{}/db/emos.sqlite", workspace_dir);
    println!("cargo:rustc-env=DATABASE_URL={}", sqlite_database_url);
}
