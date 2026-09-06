use std::fs;
use std::path::PathBuf;
use std::str::FromStr;

use futures_util::future::try_join_all;
use reqwest::IntoUrl;
use reqwest::Url;

pub(crate) fn workspace_root() -> PathBuf {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    std::path::Path::new(manifest_dir)
        .ancestors()
        .nth(2)
        .expect("Failed to find workspace root")
        .to_path_buf()
}

async fn download_file(url: impl IntoUrl, dest: PathBuf) -> anyhow::Result<()> {
    let response = reqwest::get(url).await?;
    if !response.status().is_success() {
        anyhow::bail!("Failed to download file: HTTP {}", response.status());
    }
    let bytes = response.bytes().await?;
    fs::write(dest, bytes)?;
    Ok(())
}

/// Download jpg file to 1.jpg, 2.jpg...10.jpg, if the file already exists, skip it.
pub(crate) async fn download_cover_images(
    urls: Vec<impl AsRef<str>>,
    dest_dir: &std::path::Path,
    force: bool,
) -> anyhow::Result<()> {
    let mut tasks = vec![];
    if !dest_dir.exists() {
        fs::create_dir(dest_dir)?;
    }
    let mut url_iter = urls.iter();
    for index in 1..=10 {
        let filename = dest_dir.join(format!("{}.jpg", index));
        if force || !filename.exists() {
            let url = url_iter.next();
            match url {
                None => break,
                Some(url) => {
                    let url = Url::from_str(url.as_ref())?;
                    assert!(url.path().ends_with("jpg"), "Only support download jpg");
                    tasks.push(download_file(url, filename));
                }
            }
        }
    }

    try_join_all(tasks).await?;
    Ok(())
}
