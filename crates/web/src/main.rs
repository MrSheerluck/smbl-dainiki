mod app;
mod auth;
mod billing;
mod db;
mod dodo;
mod health;
mod layout;
mod mail;
mod sync;
mod sync_storage;
mod typography;

use topcoat::{
    asset::{AssetBundle, RouterBuilderAssetExt},
    cookie::RouterBuilderCookieExt,
    font::RouterBuilderFontExt,
    router::{Router, RouterBuilderDirectoryExt, TrustedProxies},
    session::RouterBuilderSessionExt,
};

fn trusted_proxies_from_env() -> Option<TrustedProxies> {
    let networks: Vec<String> = std::env::var("TRUSTED_PROXY_NETWORKS")
        .ok()?
        .split(',')
        .map(str::trim)
        .filter(|network| !network.is_empty())
        .map(str::to_string)
        .collect();

    if networks.is_empty() {
        return None;
    }

    Some(TrustedProxies::new().networks(networks))
}

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let database = db::connect()
        .await
        .expect("could not connect to Postgres; is `docker compose up -d db` running?");

    let storage = sync_storage::SyncStorage::from_env();
    let mut builder = Router::builder()
        .app_context(database)
        .app_context(storage)
        .cookies()
        .sessions(auth::session_config())
        .layout(layout::root)
        .page(app::timeline)
        .page(auth::signup_page)
        .page(auth::signup)
        .page(auth::signin_page)
        .page(auth::signin)
        .page(auth::verify_email)
        .page(auth::verify_email_change)
        .page(auth::resend_verification)
        .page(auth::forgot_password_page)
        .page(auth::forgot_password)
        .page(auth::reset_password_page)
        .page(auth::reset_password)
        .page(auth::account)
        .page(auth::change_email)
        .page(auth::change_password)
        .page(auth::delete_account)
        .route(auth::signout)
        .page(billing::billing_page)
        .page(billing::billing_checkout)
        .route(billing::billing_return)
        .page(billing::billing_portal)
        .route(billing::billing_webhook)
        .route(sync::sync_status)
        .route(sync::setup_sync_keys)
        .route(sync::get_sync_keys)
        .route(sync::get_manifest)
        .route(sync::put_manifest)
        .route(sync::get_entry)
        .route(sync::prepare_entry_upload)
        .route(sync::commit_entry_upload)
        .route(sync::delete_entry)
        .route(health::health)
        .route(health::ready)
        .public_dir("web/client")
        .assets(AssetBundle::load().unwrap())
        .font(typography::INTER)
        .font(typography::NEWSREADER);

    if let Some(trusted_proxies) = trusted_proxies_from_env() {
        builder = builder.trusted_proxies(trusted_proxies);
    }

    let router = builder.build();

    topcoat::start(router).await.unwrap();
}
