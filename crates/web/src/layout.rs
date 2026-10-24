use crate::typography::{INTER, NEWSREADER};

use topcoat::{
    Result,
    context::Cx,
    router::{Slot, header, layout, request::uri, response::response_headers},
    view::{View, view},
};

const DEFAULT_APP_ORIGIN: &str = "https://app.dainiki.smbl.dev";
const OG_TITLE: &str = "Dainiki — a quiet place to keep your days.";
const OG_DESCRIPTION: &str =
    "A local-first, text-only daily journal. Plain Markdown on your device, with optional end-to-end encrypted sync.";

fn app_origin() -> String {
    std::env::var("APP_BASE_URL")
        .unwrap_or_else(|_| DEFAULT_APP_ORIGIN.to_string())
        .trim_end_matches('/')
        .to_string()
}

fn asset_version() -> String {
    std::env::var("DAINIKI_ASSET_VERSION").unwrap_or_else(|_| "dev".to_string())
}

#[layout("/")]
pub async fn root(cx: &Cx, slot: Slot<'_>) -> Result<impl View> {
    response_headers(cx).append(
        header::CACHE_CONTROL,
        header::HeaderValue::from_static("no-store, max-age=0"),
    );

    let path = uri(cx).path();
    let journal = path == "/";
    let origin = app_origin();
    let asset_version = asset_version();
    let page_url = format!("{origin}{path}");
    let og_image = format!("{origin}/og.png");
    let title = match path {
        "/signup" => "Create a sync account · Dainiki",
        "/signin" => "Sign in · Dainiki",
        "/verify-email" => "Verify email · Dainiki",
        "/verify-email-change" => "Confirm email change · Dainiki",
        "/forgot-password" => "Reset password · Dainiki",
        "/reset-password" => "Choose a new password · Dainiki",
        "/account" => "Account · Dainiki",
        "/billing" => "Sync & billing · Dainiki",
        "/billing/return" => "Billing · Dainiki",
        _ => "Dainiki",
    };

    Ok(view! {
        <!DOCTYPE html>
        <html
            lang="en"
            if journal {
                data-theme="light"
            }
        >
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <meta http-equiv="Cache-Control" content="no-cache, no-store, must-revalidate">
                <meta http-equiv="Pragma" content="no-cache">
                <meta http-equiv="Expires" content="0">
                <meta name="theme-color" content="#fef8f6">
                <meta name="description" content=(OG_DESCRIPTION)>
                <meta property="og:type" content="website">
                <meta property="og:url" content=(page_url.clone())>
                <meta property="og:site_name" content="Dainiki">
                <meta property="og:title" content=(OG_TITLE)>
                <meta property="og:description" content=(OG_DESCRIPTION)>
                <meta property="og:image" content=(og_image.clone())>
                <meta property="og:image:type" content="image/png">
                <meta property="og:image:width" content="1200">
                <meta property="og:image:height" content="630">
                <meta
                    property="og:image:alt"
                    content="Dainiki — a quiet place to keep your days."
                >
                <meta name="twitter:card" content="summary_large_image">
                <meta name="twitter:title" content=(OG_TITLE)>
                <meta name="twitter:description" content=(OG_DESCRIPTION)>
                <meta name="twitter:image" content=(og_image.clone())>
                <meta
                    name="twitter:image:alt"
                    content="Dainiki — a quiet place to keep your days."
                >
                <title>(title)</title>
                <link rel="canonical" href=(page_url)>
                <link rel="icon" href="/favicon.ico" sizes="any">
                <link
                    rel="icon"
                    type="image/png"
                    sizes="32x32"
                    href="/favicon-32x32.png"
                >
                <link
                    rel="icon"
                    type="image/png"
                    sizes="16x16"
                    href="/favicon-16x16.png"
                >
                <link
                    rel="apple-touch-icon"
                    sizes="180x180"
                    href="/apple-touch-icon.png"
                >
                <link rel="manifest" href="/manifest.webmanifest">
                topcoat::font::link(font: INTER)
                topcoat::font::link(font: NEWSREADER)
                <link rel="stylesheet" href=(topcoat::tailwind::stylesheet!())>
                topcoat::dev::script()
            </head>
            <body class="bg-surface font-ui text-ui-body text-on-surface antialiased">
                (slot)
                if journal {
                    <script type="module" src=(format!("/loader.js?v={asset_version}"))></script>
                }
            </body>
        </html>
    })
}
