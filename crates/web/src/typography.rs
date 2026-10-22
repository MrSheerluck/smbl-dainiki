use topcoat::{
    asset::asset,
    font::{Font, font},
};

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
        src: url(asset!("./styles/fonts/newsreader-variable.woff2")) format("woff2")
        tech("variations");
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
