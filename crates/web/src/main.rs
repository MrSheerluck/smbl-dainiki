use topcoat::{
    Result,
    router::{Router, page},
    view::{View, view},
};

#[tokio::main]
async fn main() {
    let router = Router::builder().page(home).build();

    topcoat::start(router).await.unwrap();
}

#[page("/")]
async fn home() -> Result<impl View> {
    Ok(view! {
        <!DOCTYPE html>
        <html lang="en">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <title>"Dainiki"</title>
                topcoat::dev::script()
            </head>
            <body>
                <main>
                    <p>"Local-first journal"</p>
                    <h1>"Dainiki"</h1>
                    <p>"The journal is being built. This is the first running slice."</p>
                </main>
            </body>
        </html>
    })
}
