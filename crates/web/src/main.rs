mod app;
mod auth;
mod billing;
mod db;
mod dodo;
mod health;
mod layout;
mod mail;
mod typography;

use topcoat::{
    asset::{AssetBundle, RouterBuilderAssetExt},
    cookie::RouterBuilderCookieExt,
    font::RouterBuilderFontExt,
    router::{Router, RouterBuilderDirectoryExt},
    session::RouterBuilderSessionExt,
};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let database = db::connect()
        .await
        .expect("could not connect to Postgres; is `docker compose up -d db` running?");

    let router = Router::builder()
        .app_context(database)
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
        .route(health::health)
        .route(health::ready)
        .public_dir("web/client")
        .assets(AssetBundle::load().unwrap())
        .font(typography::INTER)
        .font(typography::NEWSREADER)
        .build();

    topcoat::start(router).await.unwrap();
}
