use topcoat::{
    Result,
    router::page,
    view::{View, component, view},
};

#[page("/")]
pub async fn timeline() -> Result<impl View> {
    Ok(view! {
        <div class="flex min-h-screen bg-surface lg:h-screen lg:overflow-hidden">
            sidebar()
            <div
                id="sidebar-backdrop"
                hidden="true"
                class="fixed inset-0 z-30 bg-black/40 backdrop-blur-sm lg:hidden"
            ></div>
            <div class="flex min-w-0 flex-1 flex-col lg:min-h-0">
                topbar()
                <main
                    class="flex-1 px-4 py-10 sm:px-6 sm:py-12 lg:min-h-0 lg:overflow-y-auto lg:px-10"
                >
                    <div class="mx-auto space-y-8">
                        today_entry()
                        <div
                            id="active-filter"
                            hidden="true"
                            class="flex items-center gap-2"
                        >
                            <span
                                class="font-ui text-ui-label text-on-surface-variant/60"
                            >
                                "Filtered by"
                            </span>
                            <button
                                id="clear-filter"
                                type="button"
                                class="inline-flex items-center gap-2 rounded-full bg-surface-container-high px-3 py-1 font-mono text-ui-label text-on-surface hover:bg-surface-container"
                            >
                                <span id="active-filter-label">"#tag"</span>
                                <span aria-hidden="true">"×"</span>
                            </button>
                        </div>
                        <div id="timeline-entries" class="space-y-8"></div>
                    </div>
                </main>
            </div>
            search_overlay()
            data_overlay()
            welcome_overlay()
        </div>
    })
}

#[component]
pub async fn welcome_overlay() -> Result<impl View> {
    Ok(view! {
        <div
            id="welcome-overlay"
            hidden="true"
            role="dialog"
            aria-modal="true"
            aria-labelledby="welcome-title"
            class="fixed inset-0 z-50 flex items-center justify-center bg-black/40 px-4 backdrop-blur-sm"
        >
            <div
                class="w-full max-w-md rounded-xl border border-outline-variant/50 bg-surface-container-lowest p-6 shadow-2xl"
            >
                <p class="font-ui text-[10px] uppercase tracking-[0.16em] text-primary">
                    "Local-first journal"
                </p>
                <h2
                    id="welcome-title"
                    class="mt-2 font-display text-headline-md text-on-surface"
                >
                    "Your journal lives on this device."
                </h2>
                <p class="mt-3 font-journal text-journal text-on-surface-variant">
                    "Entries are plain Markdown files in your browser's storage. They work offline, need no account, and never touch a server."
                </p>
                <ul class="mt-4 space-y-2 font-ui text-ui-body text-on-surface-variant">
                    <li>"Markdown files you can export anytime"</li>
                    <li>"Text only — no photos, audio, or video"</li>
                    <li>"Optional encrypted sync later"</li>
                </ul>
                <button
                    id="start-writing"
                    type="button"
                    class="mt-6 w-full rounded bg-primary px-4 py-2 font-ui text-ui-label text-on-primary hover:bg-primary-container hover:text-on-primary-container"
                >
                    "Start writing"
                </button>
                <p
                    id="welcome-storage"
                    class="mt-3 text-center font-ui text-[10px] text-on-surface-variant/60"
                >
                    "We'll ask the browser to protect this storage."
                </p>
            </div>
        </div>
    })
}

#[component]
pub async fn data_overlay() -> Result<impl View> {
    Ok(view! {
        <div
            id="data-overlay"
            hidden="true"
            role="dialog"
            aria-modal="true"
            aria-label="Import and export"
            class="fixed inset-0 z-50 flex items-start justify-center bg-black/40 px-4 pt-24 backdrop-blur-sm"
        >
            <div
                class="w-full max-w-md overflow-hidden rounded-xl border border-outline-variant/50 bg-surface-container-lowest shadow-2xl"
            >
                <div
                    class="flex items-center justify-between border-b border-outline-variant/40 px-5 py-3"
                >
                    <h2 class="font-display text-lg text-on-surface">"Data"</h2>
                    <button
                        id="close-data"
                        type="button"
                        aria-label="Close data panel"
                        class="rounded px-2 py-1 font-ui text-ui-label text-on-surface-variant hover:bg-surface-container hover:text-on-surface"
                    >
                        "×"
                    </button>
                </div>
                <div class="space-y-3 p-5">
                    <button
                        id="export-zip"
                        type="button"
                        class="w-full rounded border border-outline-variant/60 px-4 py-2 text-left font-ui text-ui-body text-on-surface hover:bg-surface-container"
                    >
                        "Export journal as zip"
                    </button>
                    <button
                        id="import-zip"
                        type="button"
                        class="w-full rounded border border-outline-variant/60 px-4 py-2 text-left font-ui text-ui-body text-on-surface hover:bg-surface-container"
                    >
                        "Import zip or Markdown files"
                    </button>
                    <button
                        id="import-folder"
                        type="button"
                        class="w-full rounded border border-outline-variant/60 px-4 py-2 text-left font-ui text-ui-body text-on-surface hover:bg-surface-container"
                    >
                        "Import a folder"
                    </button>
                    <p
                        id="data-status"
                        class="font-ui text-[10px] text-on-surface-variant/60"
                    >
                        "Markdown only. Media files are ignored."
                    </p>
                    <input
                        id="import-file"
                        type="file"
                        accept=".zip,.md,text/markdown"
                        multiple="true"
                        hidden="true"
                    >
                    <input
                        id="import-folder-input"
                        type="file"
                        webkitdirectory="true"
                        multiple="true"
                        hidden="true"
                    >
                </div>
            </div>
        </div>
    })
}

#[component]
pub async fn search_overlay() -> Result<impl View> {
    Ok(view! {
        <div
            id="search-overlay"
            hidden="true"
            role="dialog"
            aria-modal="true"
            aria-label="Search entries"
            class="fixed inset-0 z-50 flex items-start justify-center bg-black/40 px-4 pt-24 backdrop-blur-sm"
        >
            <div
                class="w-full max-w-xl overflow-hidden rounded-xl border border-outline-variant/50 bg-surface-container-lowest shadow-2xl"
            >
                <input
                    id="search-input"
                    type="text"
                    placeholder="Search entries…"
                    autocomplete="off"
                    spellcheck="false"
                    class="w-full border-0 bg-transparent px-5 py-4 font-ui text-ui-body text-on-surface outline-none placeholder:text-on-surface-variant/50"
                >
                <div
                    id="search-results"
                    class="max-h-80 overflow-y-auto border-t border-outline-variant/40"
                ></div>
                <div
                    class="flex items-center justify-between border-t border-outline-variant/40 px-5 py-2 font-ui text-[10px] text-on-surface-variant/50"
                >
                    <span>"Esc to close"</span>
                    <span>"↑↓ to move · ↵ to open"</span>
                </div>
            </div>
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
                <span
                    id="today-weekday"
                    class="mt-1 block whitespace-nowrap font-ui text-[8px] uppercase tracking-wide text-on-surface-variant/70 sm:text-[9px]"
                >
                    "Today"
                </span>
                <span
                    id="today-month"
                    class="mt-0.5 block font-ui text-[8px] text-on-surface-variant/50 sm:text-[9px]"
                >
                    "Oct 2026"
                </span>
            </div>

            <div
                class="relative min-w-0 flex-1 border-l border-outline-variant/25 pl-4 sm:pl-6"
            >
                <span
                    class="absolute -left-1 top-7 h-1.5 w-1.5 rounded-full bg-outline-variant"
                ></span>
                <div
                    class="rounded-xl border border-outline-variant/40 bg-surface-container-lowest/80 p-5 shadow-sm sm:p-6"
                >
                    <div class="mb-4 flex items-center justify-end gap-2">
                        <button
                            id="prev-day"
                            type="button"
                            class="rounded px-2 py-1 font-ui text-ui-label text-on-surface-variant/70 hover:bg-surface-container hover:text-on-surface"
                            aria-label="Previous day"
                        >
                            "‹"
                        </button>
                        <button
                            id="today-button"
                            type="button"
                            hidden="true"
                            class="rounded px-2 py-1 font-ui text-ui-label text-primary hover:bg-surface-container"
                        >
                            "Today"
                        </button>
                        <button
                            id="next-day"
                            type="button"
                            class="rounded px-2 py-1 font-ui text-ui-label text-on-surface-variant/70 hover:bg-surface-container hover:text-on-surface disabled:cursor-not-allowed disabled:opacity-30"
                            aria-label="Next day"
                        >
                            "›"
                        </button>
                    </div>

                    <div
                        id="entry-editor"
                        class="live-editor min-h-[22rem] w-full font-journal text-journal text-on-surface"
                        contenteditable="true"
                        spellcheck="true"
                        role="textbox"
                        aria-multiline="true"
                        aria-label="Journal entry"
                    ></div>

                    <div
                        class="mt-4 flex flex-wrap items-center justify-between gap-3 border-t border-outline-variant/30 pt-3"
                    >
                        <div
                            id="entry-status"
                            class="font-ui text-ui-label text-on-surface-variant/70"
                            aria-live="polite"
                        >
                            "Loading…"
                        </div>
                        <div id="entry-tags" class="flex flex-wrap items-center gap-3"></div>
                    </div>
                </div>
            </div>
        </article>
    })
}

#[component]
pub async fn sidebar() -> Result<impl View> {
    Ok(view! {
        <aside
            class="fixed inset-y-0 left-0 z-40 hidden h-screen w-[230px] shrink-0 flex-col border-r border-outline-variant/40 bg-surface-container-low/70 lg:static lg:z-auto lg:flex"
        >
            <div class="min-h-0 flex-1 overflow-hidden px-3 py-4">
                <div class="flex items-center gap-3 px-2">
                    <div
                        class="flex h-9 w-9 items-center justify-center rounded-sm bg-primary font-display text-lg text-on-primary"
                    >
                        "D"
                    </div>
                    <div>
                        <p
                            class="font-ui text-sm font-semibold leading-none tracking-tight text-on-surface"
                        >
                            "Dainiki"
                        </p>
                        <p
                            class="mt-1 font-ui text-[9px] uppercase tracking-[0.14em] text-on-surface-variant"
                        >
                            "A quiet journal"
                        </p>
                    </div>
                    <button
                        id="close-sidebar"
                        type="button"
                        aria-label="Close navigation"
                        class="ml-auto rounded px-2 py-1 font-ui text-ui-label text-on-surface-variant hover:bg-surface-container hover:text-on-surface lg:hidden"
                    >
                        "×"
                    </button>
                </div>

                <nav class="mt-6 space-y-0.5">
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
                        <span
                            class="rounded bg-surface-container px-1.5 py-0.5 font-mono text-[9px] text-on-surface-variant/80"
                        >
                            "⌘K"
                        </span>
                    </button>
                </nav>

                <div class="mt-6">
                    <div class="flex items-center justify-between px-2 pb-1">
                        <h2
                            class="font-ui text-[10px] font-semibold uppercase tracking-wider text-on-surface-variant/80"
                        >
                            "Tags"
                        </h2>
                        <button
                            type="button"
                            class="font-ui text-ui-label text-on-surface-variant/70 hover:text-on-surface"
                        >
                            "+"
                        </button>
                    </div>
                    <div id="tag-list" class="space-y-0.5">
                        <p
                            class="px-2 py-1 font-ui text-[10px] text-on-surface-variant/50"
                        >
                            "Loading…"
                        </p>
                    </div>
                </div>
            </div>

            <div class="shrink-0 space-y-3 border-t border-outline-variant/40 p-3">
                <div
                    id="calendar"
                    class="rounded-lg border border-outline-variant/40 bg-surface-container-lowest px-2 py-2 shadow-sm"
                >
                    <p class="px-1 py-1 font-ui text-[10px] text-on-surface-variant/50">
                        "Loading…"
                    </p>
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
                    <span
                        class="rounded bg-surface-container px-1.5 py-0.5 font-mono text-[9px] text-on-surface-variant/70"
                    >
                        "⌘,"
                    </span>
                </div>

                <a
                    href="/signup"
                    class="flex items-center justify-between px-1 font-ui text-ui-label text-on-surface-variant hover:text-on-surface"
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
                            <path
                                d="M7 18a4 4 0 0 1-.6-7.96A6 6 0 0 1 18 8.5a4.5 4.5 0 0 1 1 8.9"
                            ></path>
                            <path d="M12 12v8"></path>
                            <path d="m8.5 15.5 3.5-3.5 3.5 3.5"></path>
                        </svg>
                        <span>"Enable sync"</span>
                    </span>
                    <span class="font-ui text-[10px] text-on-surface-variant/50">
                        "Paid"
                    </span>
                </a>

                <div class="flex items-center justify-between px-1">
                    <button
                        id="open-data"
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
                            <ellipse cx="12" cy="5" rx="8" ry="3"></ellipse>
                            <path d="M4 5v14c0 1.7 3.6 3 8 3s8-1.3 8-3V5"></path>
                            <path d="M4 12c0 1.7 3.6 3 8 3s8-1.3 8-3"></path>
                        </svg>
                        <span>"Data"</span>
                    </button>
                    <span class="font-ui text-[10px] text-on-surface-variant/50">
                        "Import / export"
                    </span>
                </div>

                <div class="flex items-center gap-1.5 px-1 text-on-surface-variant/60">
                    <span class="h-1.5 w-1.5 rounded-full bg-primary"></span>
                    <span id="storage-status" class="font-ui text-[10px]">
                        "Stored on this device"
                    </span>
                </div>
            </div>
        </aside>
    })
}

#[component]
pub async fn topbar() -> Result<impl View> {
    Ok(view! {
        <header
            class="flex h-16 shrink-0 items-center justify-between gap-4 border-b border-outline-variant/40 px-6"
        >
            <div class="flex items-center gap-2">
                <button
                    id="open-sidebar"
                    type="button"
                    aria-label="Open navigation"
                    class="rounded px-2 py-1 font-ui text-ui-label text-on-surface-variant hover:bg-surface-container hover:text-on-surface lg:hidden"
                >
                    "☰"
                </button>
                <div
                    class="hidden shrink-0 items-center gap-2 font-ui text-ui-label text-on-surface-variant/70 sm:flex"
                >
                    <span class="h-1.5 w-1.5 rounded-full bg-primary"></span>
                    "Local · saved"
                </div>
            </div>

            <div class="flex min-w-0 items-center gap-2">
                <button
                    id="open-search"
                    type="button"
                    class="flex w-full max-w-sm items-center gap-3 rounded-full border border-outline-variant/50 bg-surface-container-lowest px-4 py-2 text-left font-ui text-ui-body text-on-surface-variant hover:border-outline"
                >
                    <span>"Search entries…"</span>
                    <span
                        class="ml-auto rounded bg-surface-container px-1.5 py-0.5 font-mono text-[9px] text-on-surface-variant/80"
                    >
                        "⌘K"
                    </span>
                </button>
                <button
                    id="theme-toggle"
                    type="button"
                    aria-label="Switch to dark theme"
                    class="flex h-9 w-9 shrink-0 items-center justify-center rounded-full border border-outline-variant/50 text-on-surface-variant hover:bg-surface-container hover:text-on-surface"
                >
                    "☾"
                </button>
            </div>
        </header>
    })
}
