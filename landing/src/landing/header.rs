use topcoat::{
    Result,
    asset::asset,
    view::{View, component, view},
};

#[component]
pub async fn header() -> Result<impl View> {
    Ok(view! {
        <header
            class="fixed top-0 left-0 right-0 z-50 bg-surface/80 backdrop-blur-xl shadow-[0_1px_8px_rgba(44,42,41,0.04)]"
        >
            <div
                class="h-16 max-w-7xl mx-auto px-6 lg:px-12 flex items-center justify-between gap-gutter"
            >
                <a class="flex items-center gap-space-sm group" href="/">
                    <img
                        alt="Dainiki logo"
                        class="h-8 w-8 rounded-lg object-contain"
                        src=(asset!("./img/logo.png"))
                    >
                    <span class="flex flex-col">
                        <span
                            class="font-headline-sm text-headline-sm tracking-tight text-on-surface leading-none"
                        >
                            "Dainiki"
                        </span>
                        <span
                            class="font-ui-caption text-ui-caption text-on-surface-variant tracking-wider uppercase leading-none mt-0.5"
                        >
                            "by SMBL"
                        </span>
                    </span>
                </a>

                <nav class="hidden md:flex items-center gap-7">
                    <a
                        class="font-ui-body text-ui-body text-on-surface-variant hover:text-on-surface transition-colors"
                        href="#features"
                    >
                        "Features"
                    </a>
                    <a
                        class="font-ui-body text-ui-body text-on-surface-variant hover:text-on-surface transition-colors"
                        href="#pricing"
                    >
                        "Pricing"
                    </a>
                    <a
                        class="font-ui-body text-ui-body text-on-surface-variant hover:text-on-surface transition-colors"
                        href="#self-host"
                    >
                        "Self-host"
                    </a>
                    <a
                        class="font-ui-body text-ui-body text-on-surface-variant hover:text-on-surface transition-colors"
                        href="#faq"
                    >
                        "FAQ"
                    </a>
                    <a
                        class="font-ui-body text-ui-body text-on-surface-variant hover:text-on-surface transition-colors"
                        href="#waitlist"
                    >
                        "Waitlist"
                    </a>
                    <a
                        class="font-ui-body text-ui-body text-on-surface-variant hover:text-on-surface transition-colors"
                        href="https://github.com/MrSheerluck/smbl-dainiki"
                        rel="noopener"
                        target="_blank"
                    >
                        "GitHub"
                    </a>
                </nav>

                <div class="flex items-center gap-space-sm">
                    <button
                        type="button"
                        data-theme-toggle="true"
                        aria-label="Toggle light and dark theme"
                        title="Toggle theme"
                        class="h-9 w-9 inline-flex items-center justify-center rounded border border-outline-variant/60 text-on-surface-variant hover:text-on-surface hover:bg-surface-container transition-colors"
                    >
                        <svg
                            aria-hidden="true"
                            class="h-4 w-4"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="1.7"
                            stroke-linecap="round"
                            stroke-linejoin="round"
                        >
                            <path d="M12 3v2"></path>
                            <path d="M12 19v2"></path>
                            <path d="m5.6 5.6 1.4 1.4"></path>
                            <path d="m17 17 1.4 1.4"></path>
                            <path d="M3 12h2"></path>
                            <path d="M19 12h2"></path>
                            <path d="m5.6 18.4 1.4-1.4"></path>
                            <path d="m17 7 1.4-1.4"></path>
                            <circle cx="12" cy="12" r="4"></circle>
                        </svg>
                    </button>
                    <a
                        class="hidden sm:inline-flex items-center px-4 py-2 rounded bg-primary text-on-primary font-ui-label text-ui-label hover:bg-primary-container hover:text-on-primary-container shadow-[0_1px_2px_rgba(44,42,41,0.08)] transition-all"
                        href="/app"
                    >
                        "Start writing"
                    </a>
                </div>
            </div>
        </header>
    })
}
