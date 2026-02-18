pub fn project_root() -> std::path::PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    std::path::Path::new(manifest_dir)
        .ancestors()
        .nth(2)
        .expect("Failed to find workspace root")
        .to_path_buf()
}
