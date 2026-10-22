mod app;
mod layout;
mod typography;

use topcoat::{
    asset::{AssetBundle, RouterBuilderAssetExt},
    font::RouterBuilderFontExt,
    router::{Router, RouterBuilderDirectoryExt},
};

#[tokio::main]
async fn main() {
    let router = Router::builder()
        .layout(layout::root)
        .page(app::timeline)
        .serve_dir("/wasm/{*file}", "web/pkg")
        .assets(AssetBundle::load().unwrap())
        .font(typography::INTER)
        .font(typography::NEWSREADER)
        .build();

    topcoat::start(router).await.unwrap();
}
