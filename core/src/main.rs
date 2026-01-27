mod sync_video_list;

use dao::VideoTable;

#[tokio::main]
pub async fn main() {
    VideoTable::new().await.unwrap();
}
