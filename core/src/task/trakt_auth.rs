use anyhow::Result;
use task_macro::add_task;
use trakt_api::auth::TraktAuth;

#[add_task("trakt_auth")]
pub async fn task() -> Result<()> {
    let auth = TraktAuth::new()?;
    let authorization = auth.request_device_authorization().await?;

    println!("请打开：{}", authorization.verification_url());
    println!("输入授权码：{}", authorization.user_code());
    println!("等待 Trakt 授权…");

    auth.complete_device_authorization(authorization).await?;
    println!("Trakt 授权成功，token 已保存到 data/cache/trakt_auth.mpbr");

    Ok(())
}
