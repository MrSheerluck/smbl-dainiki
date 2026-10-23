use crate::typography::{INTER, NEWSREADER};

use topcoat::{
    Result,
    context::Cx,
    router::{Slot, layout, request::uri},
    view::{View, view},
};

#[layout("/")]
pub async fn root(cx: &Cx, slot: Slot<'_>) -> Result<impl View> {
    let path = uri(cx).path();
    let journal = path == "/";
    let title = match path {
        "/signup" => "Create a sync account · Dainiki",
        "/signin" => "Sign in · Dainiki",
        "/account" => "Account · Dainiki",
        _ => "Dainiki",
    };

    Ok(view! {
        <!DOCTYPE html>
        <html lang="en" data-theme="light">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <meta name="theme-color" content="#fef8f6">
                <title>(title)</title>
                <link rel="manifest" href="/manifest.webmanifest">
                <link rel="apple-touch-icon" href="/icon-192.png">
                topcoat::font::link(font: INTER)
                topcoat::font::link(font: NEWSREADER)
                <link rel="stylesheet" href=(topcoat::tailwind::stylesheet!())>
                topcoat::dev::script()
            </head>
            <body class="bg-surface font-ui text-ui-body text-on-surface antialiased">
                (slot)
                if journal {
                    <script type="module" src="/loader.js"></script>
                }
            </body>
        </html>
    })
}
