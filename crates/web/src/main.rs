mod app;
mod layout;

use topcoat::{
    asset::{AssetBundle, RouterBuilderAssetExt},
    router::Router,
};

#[tokio::main]
async fn main() {
    let router = Router::builder()
        .layout(layout::root)
        .page(app::timeline)
        .assets(AssetBundle::load().unwrap())
        .build();

    topcoat::start(router).await.unwrap();
}
