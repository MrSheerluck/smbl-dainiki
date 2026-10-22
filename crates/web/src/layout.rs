use crate::typography::{INTER, NEWSREADER};

use topcoat::{
    Result,
    router::{Slot, layout},
    view::{View, view},
};

#[layout("/")]
pub async fn root(slot: Slot<'_>) -> Result<impl View> {
    Ok(view! {
        <!DOCTYPE html>
        <html lang="en" data-theme="light">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <meta name="theme-color" content="#fef8f6">
                <title>"Dainiki"</title>
                topcoat::font::link(font: INTER)
                topcoat::font::link(font: NEWSREADER)
                <link rel="stylesheet" href=(topcoat::tailwind::stylesheet!())>
                topcoat::dev::script()
            </head>
            <body class="bg-surface font-ui text-ui-body text-on-surface antialiased">
                (slot)
                <script type="module" src="/client/loader.js"></script>
            </body>
        </html>
    })
}
