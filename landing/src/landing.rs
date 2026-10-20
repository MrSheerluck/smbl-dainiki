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
    context::Cx,
    font::{Font, font},
    router::{Slot, layout, page, query_params},
    view::{View, view},
};

const SITE_ORIGIN: &str = "https://dainiki.smbl.dev";

const OG_TITLE: &str = "Dainiki: a quiet, text-only journal";
const OG_DESCRIPTION: &str = "A private, local-first, text-only daily journal for the web. Plain Markdown, a calm timeline, tags, search, and a calendar. No account needed. Optional encrypted sync.";
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

#[derive(Clone, Copy)]
pub enum WaitlistStatus {
    Joined,
    Duplicate,
    Invalid,
    Error,
}

impl WaitlistStatus {
    fn from_code(code: &str) -> Option<Self> {
        match code {
            "joined" => Some(Self::Joined),
            "duplicate" => Some(Self::Duplicate),
            "invalid" => Some(Self::Invalid),
            "error" => Some(Self::Error),
            _ => None,
        }
    }

    pub fn message(self) -> &'static str {
        match self {
            Self::Joined => "You're on the list. We'll write when the journal opens.",
            Self::Duplicate => "You're already on the list.",
            Self::Invalid => "That email doesn't look right. Try again.",
            Self::Error => "Something went wrong. Try again in a moment.",
        }
    }

    pub fn tone_class(self) -> &'static str {
        match self {
            Self::Joined | Self::Duplicate => "text-primary",
            Self::Invalid | Self::Error => "text-error",
        }
    }
}

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
                    content="Dainiki: a quiet, text-only journal"
                >
                <meta name="twitter:card" content="summary_large_image">
                <meta name="twitter:title" content=(OG_TITLE)>
                <meta name="twitter:description" content=(OG_DESCRIPTION)>
                <meta name="twitter:image" content=(OG_IMAGE)>

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

#[topcoat::router::query_params(error = bad_request)]
pub struct HomeQuery {
    pub joined: Option<String>,
}

#[page("/")]
pub async fn home(cx: &Cx) -> Result<impl View> {
    let status = query_params::<HomeQuery>(cx)?
        .joined
        .as_deref()
        .and_then(WaitlistStatus::from_code);

    Ok(view! {
        <div class="flex flex-col w-full">
            header::header()
            <main class="w-full">
                hero::hero(status: status)
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

#[page("/app")]
pub async fn app_placeholder() -> Result<impl View> {
    Ok(view! {
        <div class="flex flex-col w-full">
            header::header()
            <main class="min-h-[70vh] pt-32 pb-24 px-6">
                <div class="max-w-xl mx-auto text-center space-y-6">
                    <span
                        class="inline-flex items-center gap-2 px-3 py-1 rounded-full bg-surface-container-high text-on-surface-variant font-ui-caption text-ui-caption uppercase tracking-wide"
                    >
                        <span class="w-1.5 h-1.5 rounded-full bg-primary"></span>
                        "In development"
                    </span>
                    <h1 class="font-headline-lg text-headline-lg text-on-surface">
                        "The journal is being built."
                    </h1>
                    <p
                        class="font-journal-body-lg text-journal-body-lg text-on-surface-variant leading-relaxed"
                    >
                        "The local-first editor, timeline, tags, calendar, search, and paid sync checkout are next. This page will become your writing surface."
                    </p>
                    <div
                        class="flex flex-col sm:flex-row items-center justify-center gap-3"
                    >
                        <a
                            href="/"
                            class="inline-flex items-center justify-center px-5 py-2.5 rounded bg-primary text-on-primary font-ui-label text-ui-label hover:bg-primary-container hover:text-on-primary-container transition-all"
                        >
                            "Back to the landing page"
                        </a>
                        <a
                            href="https://github.com/MrSheerluck/smbl-dainiki"
                            class="inline-flex items-center justify-center px-5 py-2.5 rounded border border-outline-variant text-on-surface font-ui-label text-ui-label hover:bg-surface-container transition-all"
                        >
                            "Follow the build"
                        </a>
                    </div>
                </div>
            </main>
            footer::footer()
        </div>
    })
}
