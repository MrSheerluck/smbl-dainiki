mod landing;
mod waitlist;

use topcoat::{
    asset::{AssetBundle, RouterBuilderAssetExt},
    font::RouterBuilderFontExt,
    router::Router,
};

#[tokio::main]
async fn main() {
    dotenvy::dotenv().ok();

    let router = Router::builder()
        .layout(landing::root_layout)
        .page(landing::home)
        .page(landing::app_placeholder)
        .route(waitlist::join)
        .assets(AssetBundle::load().unwrap())
        .font(landing::INTER)
        .font(landing::NEWSREADER)
        .app_context(waitlist::WaitlistStore::from_env())
        .build();

    topcoat::start(router).await.unwrap();
}
