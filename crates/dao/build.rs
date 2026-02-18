use std::path::Path;

fn main() {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");

    let db_path = Path::new(manifest_dir)
        .ancestors()
        .nth(2)
        .expect("Failed to find workspace root")
        .join("data/emos.sqlite");

    let sqlite_database_url = format!("sqlite://{}", db_path.to_string_lossy());

    println!("cargo:rustc-env=DATABASE_URL={}", sqlite_database_url);
    println!("cargo:rustc-env=MIGRATIONS_DIR={}/migrations", manifest_dir);
}
