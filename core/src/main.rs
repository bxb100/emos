mod sync_video_list;
mod watch_basic_genere;

use dao::Dao;

#[tokio::main]
pub async fn main() {
    Dao::new().await.unwrap();
}
