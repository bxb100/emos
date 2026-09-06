use anyhow::Result;
use trakt_client::auth::AuthClient;

pub(crate) async fn authorize_trakt() -> Result<()> {
    let auth = AuthClient::from_env(crate::files::workspace_root().join("data/cache/trakt_auth"))?;
    let authorization = auth.request_device_authorization().await?;

    println!("请打开：{}", authorization.verification_url());
    println!("输入授权码：{}", authorization.user_code());
    println!("等待 Trakt 授权…");

    auth.complete_device_authorization(authorization).await?;
    println!("Trakt 授权成功，token 已保存到 data/cache/trakt_auth.mpbr");

    Ok(())
}
