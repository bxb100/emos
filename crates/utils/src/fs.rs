use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use futures_util::future::try_join_all;
use reqwest::IntoUrl;
use reqwest::Url;

pub fn project_root() -> std::path::PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    std::path::Path::new(manifest_dir)
        .ancestors()
        .nth(2)
        .expect("Failed to find workspace root")
        .to_path_buf()
}

pub async fn download_file(url: impl IntoUrl, dest: std::path::PathBuf) -> anyhow::Result<()> {
    let response = reqwest::get(url).await?;
    if !response.status().is_success() {
        anyhow::bail!("Failed to download file: HTTP {}", response.status());
    }
    let bytes = response.bytes().await?;
    fs::write(dest, bytes)?;
    Ok(())
}

pub fn filename_from_url(url: &Url) -> Option<String> {
    let mut segments = url.path_segments()?;
    segments.next_back().map(|s| s.to_string())
}

pub async fn batch_download_imgs(
    urls: Vec<impl AsRef<str>>,
    dest_dir: &std::path::Path,
) -> anyhow::Result<()> {
    let mut tasks = vec![];
    if !dest_dir.exists() {
        fs::create_dir(dest_dir)?;
    }
    for (i, url) in urls.iter().enumerate() {
        let url = Url::from_str(url.as_ref())?;
        let save_path = PathBuf::from(filename_from_url(&url).unwrap());

        let file_name = format!(
            "{}.{}",
            i + 1,
            save_path
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("jpg")
        );
        tasks.push(download_file(url, dest_dir.join(file_name)));
    }

    try_join_all(tasks).await?;
    Ok(())
}

pub fn write_json_to_file<T: serde::Serialize>(
    data: &T,
    dest: std::path::PathBuf,
) -> anyhow::Result<()> {
    let json = serde_json::to_string(data)?;
    fs::write(dest, json)?;
    Ok(())
}
