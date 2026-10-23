pub mod faq;
pub mod features;
pub mod footer;
pub mod header;
pub mod hero;
pub mod manifesto;
pub mod markdown;
pub mod pricing;
pub mod showcase;

use topcoat::{
    Result,
    asset::asset,
    font::{Font, font},
    router::{Slot, layout, page},
    view::{View, view},
};

const SITE_ORIGIN: &str = "https://dainiki.smbl.dev";

const OG_TITLE: &str = "Dainiki — a quiet place to keep your days.";
const OG_DESCRIPTION: &str = "A private, local-first, text-only daily journal for the web. Plain Markdown, a calm timeline, tags, search, and a calendar. Yours without an account. Optional end-to-end encrypted sync.";
const OG_IMAGE: &str = "https://dainiki.smbl.dev/og.png";

pub const INTER: Font = font! {
    "Inter",
    @font-face {
        src: url(asset!("./styles/fonts/inter-variable.woff2")) format("woff2") tech(
            "variations");
            font-weight: 100 900;
            font-style: normal;
            font-display: swap;
    }
};

pub const NEWSREADER: Font = font! {
    "Newsreader",
    @font-face {
        src: url(asset!("./styles/fonts/newsreader-variable.woff2")) format("woff2"
        ) tech("variations");
        font-weight: 100 900;
        font-style: normal;
        font-display: swap;
    }
    @font-face {
        src: url(asset!("./styles/fonts/newsreader-variable-italic.woff2")) format(
            "woff2") tech("variations");
            font-weight: 100 900;
            font-style: italic;
            font-display: swap;
    }
};

#[layout("/")]
pub async fn root_layout(slot: Slot<'_>) -> Result<impl View> {
    Ok(view! {
        <!DOCTYPE html>
        <html lang="en" data-theme="light">
            <head>
                <meta charset="utf-8">
                <meta name="viewport" content="width=device-width, initial-scale=1">
                <meta name="description" content=(OG_DESCRIPTION)>
                <meta name="theme-color" content="#fef8f6">
                <title>(OG_TITLE)</title>

                <link
                    rel="icon"
                    type="image/png"
                    href=(asset!("./landing/img/logo.png"))
                >
                <link rel="apple-touch-icon" href=(asset!("./landing/img/logo.png"))>

                <meta property="og:type" content="website">
                <meta property="og:url" content=(SITE_ORIGIN)>
                <meta property="og:site_name" content="Dainiki">
                <meta property="og:title" content=(OG_TITLE)>
                <meta property="og:description" content=(OG_DESCRIPTION)>
                <meta property="og:image" content=(OG_IMAGE)>
                <meta property="og:image:width" content="1200">
                <meta property="og:image:height" content="630">
                <meta
                    property="og:image:alt"
                    content="Dainiki — a quiet place to keep your days."
                >
                <meta name="twitter:card" content="summary_large_image">
                <meta name="twitter:title" content=(OG_TITLE)>
                <meta name="twitter:description" content=(OG_DESCRIPTION)>
                <meta name="twitter:image" content=(OG_IMAGE)>
                <meta
                    name="twitter:image:alt"
                    content="Dainiki — a quiet place to keep your days."
                >

                topcoat::font::link(font: INTER)
                topcoat::font::link(font: NEWSREADER)
                <link rel="stylesheet" href=(topcoat::tailwind::stylesheet!())>
                <script src=(asset!("./scripts/theme.js"))></script>
                topcoat::dev::script()
            </head>
            <body
                class="bg-surface font-ui-body text-ui-body text-on-surface antialiased"
            >
                (slot)
            </body>
        </html>
    })
}

#[page("/")]
pub async fn home() -> Result<impl View> {
    Ok(view! {
        <div class="flex flex-col w-full">
            header::header()
            <main class="w-full">
                hero::hero()
                showcase::showcase()
                features::features()
                markdown::markdown()
                manifesto::pull_quote()
                pricing::pricing()
                faq::faq()
            </main>
            footer::footer()
        </div>
    })
}

