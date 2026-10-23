use topcoat::{
    Result,
    router::page,
    view::{View, component, view},
};

#[page("/")]
pub async fn timeline() -> Result<impl View> {
    Ok(view! {
        <div
            class="app-shell flex min-h-screen bg-surface lg:h-screen lg:overflow-hidden"
        >
            sidebar()
            <div
                id="sidebar-backdrop"
                hidden="true"
                class="fixed inset-0 z-30 bg-black/40 backdrop-blur-sm lg:hidden"
            ></div>
            <button
                id="exit-zen"
                type="button"
                class="zen-exit fixed right-5 top-5 z-40 inline-flex items-center gap-2 rounded border border-outline-variant/50 bg-surface-container-lowest/90 px-3 py-2 font-ui text-ui-label text-on-surface-variant shadow-sm backdrop-blur-md hover:border-outline-variant"
                aria-label="Exit zen mode"
                title="Exit zen mode (Esc)"
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
                    <path d="M9 3H3v6"></path>
                    <path d="M15 3h6v6"></path>
                    <path d="M9 21H3v-6"></path>
                    <path d="M15 21h6v-6"></path>
                </svg>
                <span>"Exit zen"</span>
            </button>
            <div class="flex min-w-0 flex-1 flex-col lg:min-h-0">
                topbar()
                <main
                    class="app-main flex-1 px-4 py-10 sm:px-6 sm:py-12 lg:min-h-0 lg:overflow-y-auto lg:px-10"
                >
                    <div class="journal-column mx-auto max-w-5xl space-y-8">
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
            sync_overlay()
            archive_overlay()
            conflict_overlay()
            settings_overlay()
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
            class="overlay-backdrop fixed inset-0 z-50 flex items-center justify-center bg-black/40 px-4 backdrop-blur-sm"
        >
            <div
                class="overlay-card w-full max-w-md rounded-xl border border-outline-variant/50 bg-surface-container-lowest p-6 shadow-2xl"
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
pub async fn sync_overlay() -> Result<impl View> {
    Ok(view! {
        <div
            id="sync-overlay"
            hidden="true"
            role="dialog"
            aria-modal="true"
            aria-labelledby="sync-title"
            class="overlay-backdrop fixed inset-0 z-50 flex items-start justify-center bg-black/40 px-4 pt-24 backdrop-blur-sm"
        >
            <div
                class="overlay-card w-full max-w-lg overflow-hidden rounded-xl border border-outline-variant/50 bg-surface-container-lowest shadow-2xl"
            >
                <div
                    class="flex items-center justify-between border-b border-outline-variant/40 px-5 py-3"
                >
                    <div>
                        <p
                            class="font-ui text-[10px] uppercase tracking-[0.16em] text-primary"
                        >
                            "End-to-end encrypted"
                        </p>
                        <h2
                            id="sync-title"
                            class="font-display text-lg text-on-surface"
                        >
                            "Dainiki Sync"
                        </h2>
                    </div>
                    <button
                        id="close-sync"
                        type="button"
                        aria-label="Close sync panel"
                        class="rounded px-2 py-1 font-ui text-ui-label text-on-surface-variant hover:bg-surface-container hover:text-on-surface"
                    >
                        "×"
                    </button>
                </div>
                <div id="sync-content" class="p-5">
                    <p class="font-ui text-ui-label text-on-surface-variant/60">
                        "Loading…"
                    </p>
                </div>
            </div>
        </div>
    })
}

#[component]
pub async fn settings_overlay() -> Result<impl View> {
    Ok(view! {
        <div
            id="settings-overlay"
            hidden="true"
            role="dialog"
            aria-modal="true"
            aria-labelledby="settings-title"
            class="overlay-backdrop fixed inset-0 z-50 flex items-start justify-center bg-black/40 px-4 pt-20 backdrop-blur-sm sm:pt-24"
        >
            <div
                class="overlay-card w-full max-w-lg overflow-hidden rounded-xl border border-outline-variant/50 bg-surface-container-lowest shadow-2xl"
            >
                <div
                    class="flex items-start justify-between gap-4 border-b border-outline-variant/40 px-5 py-4"
                >
                    <div>
                        <p
                            class="font-ui text-[9px] uppercase tracking-[0.16em] text-primary"
                        >
                            "Preferences"
                        </p>
                        <h2
                            id="settings-title"
                            class="mt-1 font-display text-xl text-on-surface"
                        >
                            "Settings"
                        </h2>
                    </div>
                    <button
                        id="close-settings"
                        type="button"
                        aria-label="Close settings"
                        class="rounded px-2 py-1 font-ui text-ui-label text-on-surface-variant hover:bg-surface-container hover:text-on-surface"
                    >
                        "×"
                    </button>
                </div>

                <div class="max-h-[calc(100vh-11rem)] space-y-6 overflow-y-auto p-5">
                    <section>
                        <h3
                            class="font-ui text-[9px] font-semibold uppercase tracking-[0.14em] text-on-surface-variant/60"
                        >
                            "Appearance"
                        </h3>
                        <div class="mt-2 grid grid-cols-3 gap-2">
                            <button
                                type="button"
                                data-setting-theme="light"
                                aria-pressed="false"
                                class="settings-choice"
                            >
                                "Light"
                            </button>
                            <button
                                type="button"
                                data-setting-theme="dark"
                                aria-pressed="false"
                                class="settings-choice"
                            >
                                "Dark"
                            </button>
                            <button
                                type="button"
                                data-setting-theme="system"
                                aria-pressed="false"
                                class="settings-choice"
                            >
                                "System"
                            </button>
                        </div>
                        <p class="mt-2 font-ui text-[10px] text-on-surface-variant/60">
                            "System follows your device appearance."
                        </p>
                    </section>

                    <section>
                        <h3
                            class="font-ui text-[9px] font-semibold uppercase tracking-[0.14em] text-on-surface-variant/60"
                        >
                            "Journal type"
                        </h3>
                        <div class="mt-2 grid grid-cols-3 gap-2">
                            <button
                                type="button"
                                data-setting-size="compact"
                                aria-pressed="false"
                                class="settings-choice"
                            >
                                "Compact"
                            </button>
                            <button
                                type="button"
                                data-setting-size="default"
                                aria-pressed="false"
                                class="settings-choice"
                            >
                                "Default"
                            </button>
                            <button
                                type="button"
                                data-setting-size="large"
                                aria-pressed="false"
                                class="settings-choice"
                            >
                                "Large"
                            </button>
                        </div>
                    </section>

                    <section class="flex items-start justify-between gap-4">
                        <div>
                            <h3
                                class="font-ui text-[9px] font-semibold uppercase tracking-[0.14em] text-on-surface-variant/60"
                            >
                                "Spellcheck"
                            </h3>
                            <p
                                class="mt-1 font-ui text-[10px] text-on-surface-variant/60"
                            >
                                "Check spelling while you write."
                            </p>
                        </div>
                        <input
                            id="setting-spellcheck"
                            type="checkbox"
                            class="mt-1 h-4 w-4 accent-primary"
                        >
                    </section>

                    <div class="border-t border-outline-variant/40 pt-4">
                        <div class="grid grid-cols-2 gap-2">
                            <button
                                type="button"
                                data-settings-action="archive"
                                class="settings-link"
                            >
                                "Browse archive"
                            </button>
                            <button
                                type="button"
                                data-settings-action="sync"
                                class="settings-link"
                            >
                                "Sync & plan"
                            </button>
                            <button
                                type="button"
                                data-settings-action="data"
                                class="settings-link"
                            >
                                "Import / export"
                            </button>
                            <button
                                type="button"
                                data-settings-action="conflicts"
                                class="settings-link"
                            >
                                "Conflict copies"
                            </button>
                            <a href="/account" class="settings-link">"Account"</a>
                            <a href="/billing" class="settings-link">"Billing"</a>
                        </div>
                    </div>

                    <p class="font-ui text-[10px] text-on-surface-variant/50">
                        "Preferences save automatically on this device."
                    </p>
                </div>
            </div>
        </div>
    })
}

#[component]
pub async fn conflict_overlay() -> Result<impl View> {
    Ok(view! {
        <div
            id="conflict-overlay"
            hidden="true"
            role="dialog"
            aria-modal="true"
            aria-labelledby="conflict-title"
            class="overlay-backdrop fixed inset-0 z-50 flex items-start justify-center bg-black/40 px-4 pt-20 backdrop-blur-sm sm:pt-24"
        >
            <div
                class="overlay-card w-full max-w-4xl overflow-hidden rounded-xl border border-outline-variant/50 bg-surface-container-lowest shadow-2xl"
            >
                <div
                    class="flex items-start justify-between gap-4 border-b border-outline-variant/40 px-5 py-4"
                >
                    <div>
                        <p
                            class="font-ui text-[9px] uppercase tracking-[0.16em] text-secondary"
                        >
                            "Conflict copies"
                        </p>
                        <h2
                            id="conflict-title"
                            class="mt-1 font-display text-xl text-on-surface"
                        >
                            "Review before choosing"
                        </h2>
                        <p
                            class="mt-1 font-ui text-ui-caption text-on-surface-variant/70"
                        >
                            "Both versions are kept. The current entry remains canonical until you choose."
                        </p>
                    </div>
                    <button
                        id="close-conflicts"
                        type="button"
                        aria-label="Close conflict copies"
                        class="rounded px-2 py-1 font-ui text-ui-label text-on-surface-variant hover:bg-surface-container hover:text-on-surface"
                    >
                        "×"
                    </button>
                </div>
                <div
                    id="conflict-content"
                    class="max-h-[calc(100vh-11rem)] overflow-y-auto p-5"
                ></div>
                <div
                    id="conflict-status"
                    aria-live="polite"
                    class="border-t border-outline-variant/40 px-5 py-2 font-ui text-ui-caption text-on-surface-variant/70"
                ></div>
            </div>
        </div>
    })
}

#[component]
pub async fn archive_overlay() -> Result<impl View> {
    Ok(view! {
        <div
            id="archive-overlay"
            hidden="true"
            role="dialog"
            aria-modal="true"
            aria-labelledby="archive-title"
            class="overlay-backdrop fixed inset-0 z-50 flex items-start justify-center bg-black/40 px-4 pt-20 backdrop-blur-sm sm:pt-24"
        >
            <div
                class="overlay-card w-full max-w-3xl overflow-hidden rounded-xl border border-outline-variant/50 bg-surface-container-lowest shadow-2xl"
            >
                <div
                    class="flex items-start justify-between gap-4 border-b border-outline-variant/40 px-5 py-4"
                >
                    <div>
                        <p
                            class="font-ui text-[9px] uppercase tracking-[0.16em] text-primary"
                        >
                            "Archive"
                        </p>
                        <h2
                            id="archive-title"
                            class="mt-1 font-display text-xl text-on-surface"
                        >
                            "Browse the journal"
                        </h2>
                        <p
                            class="mt-1 font-ui text-ui-caption text-on-surface-variant/70"
                        >
                            "Years, months, and days."
                        </p>
                    </div>
                    <button
                        id="close-archive"
                        type="button"
                        aria-label="Close archive"
                        class="rounded px-2 py-1 font-ui text-ui-label text-on-surface-variant hover:bg-surface-container hover:text-on-surface"
                    >
                        "×"
                    </button>
                </div>
                <div
                    id="archive-content"
                    class="max-h-[calc(100vh-10rem)] overflow-y-auto p-5"
                >
                    <p class="font-ui text-ui-label text-on-surface-variant/60">
                        "Loading…"
                    </p>
                </div>
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
            class="overlay-backdrop fixed inset-0 z-50 flex items-start justify-center bg-black/40 px-4 pt-24 backdrop-blur-sm"
        >
            <div
                class="overlay-card w-full max-w-md overflow-hidden rounded-xl border border-outline-variant/50 bg-surface-container-lowest shadow-2xl"
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
                    <div class="border-t border-outline-variant/40 pt-3">
                        <div class="flex items-baseline justify-between gap-3">
                            <h3
                                class="font-ui text-[10px] font-semibold uppercase tracking-[0.14em] text-on-surface-variant/70"
                            >
                                "Entries"
                            </h3>
                            <span
                                id="data-entry-count"
                                class="font-ui text-[10px] text-on-surface-variant/50"
                            >
                                "0 entries"
                            </span>
                        </div>
                        <p
                            class="mt-1 font-ui text-[10px] leading-4 text-on-surface-variant/60"
                        >
                            "Open or delete a specific entry. Deletions sync when paid sync is on."
                        </p>
                        <div
                            id="data-entries"
                            aria-label="Saved entries"
                            class="mt-2 max-h-64 overflow-y-auto overscroll-contain rounded border border-outline-variant/40"
                        >
                            <p
                                class="px-3 py-3 font-ui text-[10px] text-on-surface-variant/50"
                            >
                                "Loading…"
                            </p>
                        </div>
                    </div>
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
            class="overlay-backdrop fixed inset-0 z-50 flex items-start justify-center bg-black/40 px-4 pt-24 backdrop-blur-sm"
        >
            <div
                class="overlay-card w-full max-w-xl overflow-hidden rounded-xl border border-outline-variant/50 bg-surface-container-lowest shadow-2xl"
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
            <div class="w-14 shrink-0 pt-1 text-right sm:w-20">
                <span
                    id="today-day"
                    class="block font-display text-[2.4rem] leading-none text-on-surface/70 sm:text-[3rem]"
                >
                    "22"
                </span>
                <span
                    id="today-weekday"
                    class="mt-1 block whitespace-nowrap font-ui text-[9px] uppercase tracking-wide text-on-surface-variant/70 sm:text-[10px]"
                >
                    "Today"
                </span>
                <span
                    id="today-month"
                    class="mt-0.5 block font-ui text-[9px] text-on-surface-variant/50 sm:text-[10px]"
                >
                    "Oct 2026"
                </span>
            </div>

            <div
                class="relative min-w-0 flex-1 border-l border-outline-variant/25 pl-4 sm:pl-5"
            >
                <span class="absolute -left-2 top-7 h-px w-3 bg-outline-variant/60"></span>
                <div
                    class="journal-page rounded-xl border border-surface-container-high/50 bg-surface-container-lowest/80 p-5 shadow-[0_1px_3px_rgba(44,42,41,0.04)] sm:p-6"
                >
                    <div
                        id="entry-editor"
                        class="live-editor min-h-[14rem] w-full font-journal text-journal text-on-surface sm:min-h-[20rem]"
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
            class="app-sidebar fixed inset-y-0 left-0 z-40 hidden h-screen w-[230px] shrink-0 flex-col border-r border-outline-variant/40 bg-surface-container-low/70 lg:static lg:z-auto lg:flex"
        >
            <div class="min-h-0 flex-1 overflow-hidden px-3 py-4">
                <div class="flex items-center gap-3 px-2">
                    <img
                        alt="Dainiki logo"
                        class="h-9 w-9 rounded object-contain"
                        src="/icon-192.png"
                    >
                    <div>
                        <p
                            class="font-ui text-sm font-semibold leading-none tracking-tight text-on-surface"
                        >
                            "Dainiki"
                        </p>
                        <p
                            class="mt-1 font-ui text-[9px] uppercase tracking-[0.14em] text-on-surface-variant"
                        >
                            "by SMBL"
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
                        <span
                            class="font-ui text-[8px] uppercase tracking-[0.14em] text-primary/70"
                        >
                            "Now"
                        </span>
                    </a>
                    <button
                        id="open-search-sidebar"
                        type="button"
                        class="sidebar-action flex w-full items-center justify-between rounded px-2 py-1.5 text-left font-ui text-ui-label text-on-surface-variant transition-colors hover:bg-surface-container-high hover:text-on-surface"
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
                    <div class="px-2 pb-1">
                        <h2
                            class="font-ui text-[10px] font-semibold uppercase tracking-wider text-on-surface-variant/80"
                        >
                            "Tags"
                        </h2>
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
                        id="open-settings"
                        type="button"
                        class="sidebar-action flex items-center gap-2 font-ui text-ui-label text-on-surface-variant hover:text-on-surface"
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

                <button
                    id="open-conflicts"
                    type="button"
                    hidden="true"
                    class="w-full rounded-xl border border-secondary/40 bg-secondary/5 px-2.5 py-2 text-left transition-colors hover:border-secondary/70"
                >
                    <span class="flex items-center justify-between gap-2">
                        <span
                            class="font-ui text-[9px] font-semibold uppercase tracking-[0.12em] text-secondary"
                        >
                            "Conflict copies"
                        </span>
                        <span
                            id="conflict-count"
                            class="font-mono text-[10px] text-secondary"
                        >
                            "0"
                        </span>
                    </span>
                    <span
                        class="mt-1 block font-ui text-[10px] leading-4 text-on-surface-variant/70"
                    >
                        "Review and resolve"
                    </span>
                </button>

                <button
                    id="open-sync"
                    type="button"
                    data-sync-state="off"
                    aria-haspopup="dialog"
                    class="sync-offer w-full px-1 pt-3 text-left"
                >
                    <span
                        id="sync-label"
                        data-sync-state="off"
                        class="block truncate font-ui text-[9px] font-semibold uppercase tracking-[0.14em] text-primary/75"
                    >
                        "Dainiki Sync"
                    </span>
                    <span
                        id="sync-offer-title"
                        class="mt-1.5 block font-display text-[1.02rem] leading-[1.2] tracking-[-0.015em] text-on-surface"
                    >
                        "Keep your journal on every device."
                    </span>
                    <span
                        id="sync-detail"
                        class="mt-1.5 block font-ui text-[10px] leading-4 text-on-surface-variant/75"
                    >
                        "End-to-end encrypted. $2/month or $20/year."
                    </span>
                    <span
                        id="sync-cta"
                        class="mt-2.5 inline-block font-ui text-[10px] text-primary underline decoration-primary/35 underline-offset-4"
                    >
                        "Sign in or sign up"
                    </span>
                </button>

                <div class="px-1 font-ui text-[10px] text-on-surface-variant/60">
                    <span
                        class="block text-[9px] uppercase tracking-[0.12em] text-primary/60"
                    >
                        "Device"
                    </span>
                    <span id="storage-status" class="mt-0.5 block leading-4">
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
            class="app-topbar flex h-16 shrink-0 items-center justify-between gap-4 border-b border-outline-variant/40 px-6"
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
            </div>

            <div class="flex min-w-0 items-center gap-2">
                <button
                    id="open-search"
                    type="button"
                    class="flex w-full max-w-sm items-center gap-3 rounded-full border border-surface-container bg-surface-container-low px-4 py-2 text-left font-ui text-ui-body text-on-surface-variant hover:border-outline-variant"
                >
                    <span>"Search entries…"</span>
                    <span
                        class="ml-auto rounded bg-surface-container px-1.5 py-0.5 font-mono text-[9px] text-on-surface-variant/80"
                    >
                        "⌘K"
                    </span>
                </button>
                <button
                    id="open-archive"
                    type="button"
                    aria-label="Browse the journal archive"
                    class="inline-flex h-9 shrink-0 items-center gap-2 rounded border border-outline-variant/60 px-3 font-ui text-ui-label text-on-surface-variant transition-colors hover:bg-surface-container hover:text-on-surface"
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
                        <rect x="3.5" y="5" width="17" height="16" rx="2"></rect>
                        <path d="M8 3v4"></path>
                        <path d="M16 3v4"></path>
                        <path d="M3.5 10h17"></path>
                        <path d="M8 14h.01"></path>
                        <path d="M12 14h.01"></path>
                        <path d="M16 14h.01"></path>
                        <path d="M8 17.5h.01"></path>
                        <path d="M12 17.5h.01"></path>
                    </svg>
                    <span class="hidden sm:inline">"Archive"</span>
                </button>

                <button
                    id="toggle-zen"
                    type="button"
                    aria-label="Enter zen mode"
                    title="Zen mode (⌘.)"
                    class="inline-flex h-9 shrink-0 items-center gap-2 rounded border border-outline-variant/60 px-3 font-ui text-ui-label text-on-surface-variant transition-colors hover:bg-surface-container hover:text-on-surface"
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
                        <path d="M4 9V5a1 1 0 0 1 1-1h4"></path>
                        <path d="M15 4h4a1 1 0 0 1 1 1v4"></path>
                        <path d="M20 15v4a1 1 0 0 1-1 1h-4"></path>
                        <path d="M9 20H5a1 1 0 0 1-1-1v-4"></path>
                    </svg>
                    <span class="hidden sm:inline">"Zen"</span>
                </button>

                <button
                    id="theme-toggle"
                    type="button"
                    aria-label="Toggle light and dark theme"
                    aria-pressed="false"
                    title="Toggle theme"
                    class="relative inline-flex h-9 w-9 shrink-0 items-center justify-center rounded border border-outline-variant/60 text-on-surface-variant transition-colors hover:bg-surface-container hover:text-on-surface"
                >
                    <svg
                        aria-hidden="true"
                        class="theme-icon theme-icon-sun h-4 w-4"
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
                    <svg
                        aria-hidden="true"
                        class="theme-icon theme-icon-moon h-4 w-4"
                        viewBox="0 0 24 24"
                        fill="none"
                        stroke="currentColor"
                        stroke-width="1.7"
                        stroke-linecap="round"
                        stroke-linejoin="round"
                    >
                        <path d="M21 12.8A8.5 8.5 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8Z"></path>
                    </svg>
                </button>
            </div>
        </header>
    })
}
