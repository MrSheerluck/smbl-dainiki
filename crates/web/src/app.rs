use topcoat::{
    Result,
    router::page,
    view::{View, class, component, view},
};

struct Tag {
    name: &'static str,
    count: &'static str,
}

const TAGS: &[Tag] = &[
    Tag {
        name: "reflections",
        count: "42",
    },
    Tag {
        name: "travel",
        count: "18",
    },
    Tag {
        name: "morning",
        count: "64",
    },
    Tag {
        name: "work",
        count: "31",
    },
    Tag {
        name: "ideas",
        count: "15",
    },
];

#[page("/")]
pub async fn timeline() -> Result<impl View> {
    Ok(view! {
        <div class="flex min-h-screen bg-surface">
            sidebar()
            <div class="flex min-w-0 flex-1 flex-col">
                topbar()
                <main class="flex-1 px-4 py-10 sm:px-6 sm:py-12 lg:px-10">
                    <div class="mx-auto space-y-8">
                        today_entry()
                        older_entry()
                    </div>
                </main>
            </div>
        </div>
    })
}

#[component]
pub async fn date_column(day: &str, weekday: &str, month: &str) -> Result<impl View> {
    Ok(view! {
        <div class="w-14 shrink-0 pt-1 text-right">
            <span class="block font-display text-[1.9rem] leading-none text-on-surface/70 sm:text-[2.2rem]">
                (day)
            </span>
            <span class="mt-1 block whitespace-nowrap font-ui text-[8px] uppercase tracking-wide text-on-surface-variant/70 sm:text-[9px]">
                (weekday)
            </span>
            <span class="mt-0.5 block font-ui text-[8px] text-on-surface-variant/50 sm:text-[9px]">
                (month)
            </span>
        </div>
    })
}

#[component]
pub async fn today_entry() -> Result<impl View> {
    Ok(view! {
        <article class="flex gap-3 sm:gap-5">
            <div class="w-14 shrink-0 pt-1 text-right">
                <span
                    id="today-day"
                    class="block font-display text-[1.9rem] leading-none text-on-surface/70 sm:text-[2.2rem]"
                >
                    "22"
                </span>
                <span class="mt-1 block whitespace-nowrap font-ui text-[8px] uppercase tracking-wide text-on-surface-variant/70 sm:text-[9px]">
                    "Today"
                </span>
                <span
                    id="today-month"
                    class="mt-0.5 block font-ui text-[8px] text-on-surface-variant/50 sm:text-[9px]"
                >
                    "Oct 2026"
                </span>
            </div>

            <div class="relative min-w-0 flex-1 border-l border-outline-variant/25 pl-4 sm:pl-6">
                <span class="absolute -left-1 top-7 h-1.5 w-1.5 rounded-full bg-outline-variant"></span>
                <div class="rounded-xl border border-outline-variant/40 bg-surface-container-lowest/80 p-5 shadow-sm sm:p-6">
                    <textarea
                        id="entry-editor"
                        class="min-h-[22rem] w-full resize-none border-0 bg-transparent font-journal text-journal text-on-surface outline-none placeholder:text-on-surface-variant/40"
                        placeholder="Write today's entry…"
                        spellcheck="true"
                    ></textarea>

                    <div class="mt-4 flex flex-wrap items-center justify-between gap-3 border-t border-outline-variant/30 pt-3">
                        <div
                            id="entry-status"
                            class="font-ui text-ui-label text-on-surface-variant/70"
                        >
                            "Loading…"
                        </div>
                        <div
                            id="entry-tags"
                            class="flex flex-wrap items-center gap-3"
                        ></div>
                    </div>
                </div>
            </div>
        </article>
    })
}

#[component]
pub async fn older_entry() -> Result<impl View> {
    Ok(view! {
        <article class="flex gap-3 sm:gap-5">
            date_column(day: "21", weekday: "Wednesday", month: "Oct 2026")
            <div class="relative min-w-0 flex-1 border-l border-outline-variant/25 pl-4 sm:pl-6">
                <span class="absolute -left-1 top-7 h-1.5 w-1.5 rounded-full bg-outline-variant"></span>
                <div class="rounded-xl border border-outline-variant/40 bg-surface-container-lowest/80 p-5 shadow-sm sm:p-6">
                    <h2 class="font-display text-headline-md tracking-tight text-on-surface">
                        "A shorter walk than planned"
                    </h2>
                    <p class="mt-3 font-journal text-journal text-on-surface">
                        "The path was muddy from the night's rain, so I turned back early and watched the river from the bridge instead. A small detour, but the right one."
                    </p>
                    <div class="mt-5 flex flex-wrap items-center gap-3">
                        tag_label(label: "#work")
                    </div>
                </div>
            </div>
        </article>
    })
}

#[component]
pub async fn tag_label(label: &str) -> Result<impl View> {
    Ok(view! {
        <span class="font-mono text-ui-label text-secondary/70">(label)</span>
    })
}

#[component]
pub async fn sidebar() -> Result<impl View> {
    Ok(view! {
        <aside class="hidden w-[230px] shrink-0 flex-col border-r border-outline-variant/40 bg-surface-container-low/70 lg:flex">
            <div class="flex-1 overflow-y-auto px-3 py-5">
                <div class="flex items-center gap-3 px-2">
                    <div class="flex h-9 w-9 items-center justify-center rounded-sm bg-primary font-display text-lg text-on-primary">
                        "D"
                    </div>
                    <div>
                        <p class="font-ui text-sm font-semibold leading-none tracking-tight text-on-surface">
                            "Dainiki"
                        </p>
                        <p class="mt-1 font-ui text-[9px] uppercase tracking-[0.14em] text-on-surface-variant">
                            "A quiet journal"
                        </p>
                    </div>
                </div>

                <nav class="mt-8 space-y-0.5">
                    <a
                        href="/"
                        class="flex items-center justify-between rounded bg-surface-container-high px-2 py-1.5 font-ui text-ui-label font-medium text-on-surface"
                    >
                        <span class="flex items-center gap-2">
                            <svg
                                aria-hidden="true"
                                class="h-4 w-4 text-primary"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="1.7"
                                stroke-linecap="round"
                            >
                                <path d="M4 6h16"></path>
                                <path d="M4 12h16"></path>
                                <path d="M4 18h16"></path>
                            </svg>
                            <span>"Timeline"</span>
                        </span>
                        <span class="h-1.5 w-1.5 rounded-full bg-primary"></span>
                    </a>
                    <button
                        type="button"
                        class="flex w-full items-center justify-between rounded px-2 py-1.5 text-left font-ui text-ui-label text-on-surface-variant transition-colors hover:bg-surface-container-high hover:text-on-surface"
                    >
                        <span class="flex items-center gap-2">
                            <svg
                                aria-hidden="true"
                                class="h-4 w-4"
                                viewBox="0 0 24 24"
                                fill="none"
                                stroke="currentColor"
                                stroke-width="1.7"
                                stroke-linecap="round"
                            >
                                <circle cx="11" cy="11" r="6"></circle>
                                <path d="m16 16 4 4"></path>
                            </svg>
                            <span>"Search"</span>
                        </span>
                        <span class="rounded bg-surface-container px-1.5 py-0.5 font-mono text-[9px] text-on-surface-variant/80">
                            "⌘K"
                        </span>
                    </button>
                </nav>

                <div class="mt-7">
                    <div class="flex items-center justify-between px-2 pb-1">
                        <h2 class="font-ui text-[10px] font-semibold uppercase tracking-wider text-on-surface-variant/80">
                            "Tags"
                        </h2>
                        <button
                            type="button"
                            class="font-ui text-ui-label text-on-surface-variant/70 hover:text-on-surface"
                        >
                            "+"
                        </button>
                    </div>
                    <div class="space-y-0.5">
                        for tag in TAGS {
                            <button
                                type="button"
                                class="flex w-full items-center justify-between rounded px-2 py-1 text-left font-ui text-ui-label text-on-surface-variant transition-colors hover:bg-surface-container-high"
                            >
                                <span class="flex items-center gap-1.5">
                                    <span class="font-mono text-secondary/70">"#"</span>
                                    (tag.name)
                                </span>
                                <span class="font-ui text-[10px] text-on-surface-variant/60">
                                    (tag.count)
                                </span>
                            </button>
                        }
                    </div>
                </div>
            </div>

            <div class="space-y-3 border-t border-outline-variant/40 p-3">
                <div class="rounded-lg border border-outline-variant/40 bg-surface-container-lowest px-2 py-2 shadow-sm">
                    <div class="flex items-center justify-between px-1 pb-1">
                        <span class="font-ui text-[10px] font-semibold text-on-surface">
                            "October 2026"
                        </span>
                        <div class="flex items-center gap-1 text-on-surface-variant">
                            <span class="text-xs leading-none">"‹"</span>
                            <span class="text-xs leading-none">"›"</span>
                        </div>
                    </div>
                    <div class="grid grid-cols-7 gap-1 text-center font-ui text-[9px] text-on-surface-variant">
                        <span>"M"</span>
                        <span>"T"</span>
                        <span>"W"</span>
                        <span>"T"</span>
                        <span>"F"</span>
                        <span>"S"</span>
                        <span>"S"</span>
                    </div>
                    <div class="grid grid-cols-7 gap-1 pt-1 text-center font-ui text-[10px]">
                        <span class="text-on-surface-variant/30">"28"</span>
                        <span class="text-on-surface-variant/30">"29"</span>
                        <span class="text-on-surface-variant/30">"30"</span>
                        for day in 1..=31u8 {
                            <span
                                class=(class!(
                                    "text-on-surface-variant" if day != 22,
                                    "font-semibold text-primary relative after:content-[''] after:absolute after:bottom-0 after:left-1/2 after:-translate-x-1/2 after:w-1 after:h-1 after:rounded-full after:bg-primary" if day == 22,
                                ))
                            >
                                (day)
                            </span>
                        }
                    </div>
                </div>

                <div class="flex items-center justify-between px-1">
                    <button
                        type="button"
                        class="flex items-center gap-2 font-ui text-ui-label text-on-surface-variant hover:text-on-surface"
                    >
                        <svg
                            aria-hidden="true"
                            class="h-4 w-4"
                            viewBox="0 0 24 24"
                            fill="none"
                            stroke="currentColor"
                            stroke-width="1.7"
                            stroke-linecap="round"
                        >
                            <path d="M4 6h9"></path>
                            <path d="M17 6h3"></path>
                            <circle cx="15" cy="6" r="2"></circle>
                            <path d="M4 12h3"></path>
                            <path d="M11 12h9"></path>
                            <circle cx="9" cy="12" r="2"></circle>
                            <path d="M4 18h9"></path>
                            <path d="M17 18h3"></path>
                            <circle cx="15" cy="18" r="2"></circle>
                        </svg>
                        <span>"Settings"</span>
                    </button>
                    <span class="rounded bg-surface-container px-1.5 py-0.5 font-mono text-[9px] text-on-surface-variant/70">
                        "⌘,"
                    </span>
                </div>

                <div class="flex items-center gap-1.5 px-1 text-on-surface-variant/60">
                    <span class="h-1.5 w-1.5 rounded-full bg-primary"></span>
                    <span class="font-ui text-[10px]">"Stored on this device"</span>
                </div>
            </div>
        </aside>
    })
}

#[component]
pub async fn topbar() -> Result<impl View> {
    Ok(view! {
        <header class="flex h-16 items-center justify-between gap-4 border-b border-outline-variant/40 px-6">
            <div class="flex shrink-0 items-center gap-2 font-ui text-ui-label text-on-surface-variant/70">
                <span class="h-1.5 w-1.5 rounded-full bg-primary"></span>
                "Local · saved"
            </div>

            <button
                type="button"
                class="flex w-full max-w-sm items-center gap-3 rounded-full border border-outline-variant/50 bg-surface-container-lowest px-4 py-2 text-left font-ui text-ui-body text-on-surface-variant hover:border-outline"
            >
                <span>"Search entries…"</span>
                <span class="ml-auto rounded bg-surface-container px-1.5 py-0.5 font-mono text-[9px] text-on-surface-variant/80">
                    "⌘K"
                </span>
            </button>
        </header>
    })
}
