use video_db::VideoTable;

#[tokio::main]
pub async fn main() {
    VideoTable::new().await.unwrap();
}
