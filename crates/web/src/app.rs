use topcoat::{
    Result,
    router::page,
    view::{View, component, view},
};

#[page("/")]
pub async fn timeline() -> Result<impl View> {
    Ok(view! {
        <div class="flex min-h-screen bg-surface">
            sidebar()
            <div class="flex min-w-0 flex-1 flex-col">
                topbar()
                <main class="flex-1 px-6 py-12 lg:px-10">
                    <div class="mx-auto max-w-3xl">
                        <p class="font-ui text-ui-label uppercase tracking-[0.16em] text-on-surface-variant">
                            "Thursday, October 22"
                        </p>
                        <h1 class="mt-3 font-display text-headline-lg text-on-surface">
                            "The journal starts here."
                        </h1>
                        <p class="mt-4 max-w-xl font-journal text-journal-lg leading-relaxed text-on-surface-variant">
                            "The shell is in place. The timeline, editor, tags, calendar, and search arrive in the next lessons."
                        </p>
                        <div class="mt-10 rounded-sm border border-dashed border-outline-variant bg-surface-container-lowest px-6 py-12 text-center">
                            <p class="font-ui text-ui-body text-on-surface-variant">
                                "Timeline and editor land here next."
                            </p>
                        </div>
                    </div>
                </main>
            </div>
        </div>
    })
}

#[component]
pub async fn sidebar() -> Result<impl View> {
    Ok(view! {
        <aside class="hidden w-64 shrink-0 flex-col border-r border-outline-variant/70 bg-surface-container-low px-4 py-6 lg:flex">
            <div class="flex items-center gap-3 px-2">
                <div class="flex h-9 w-9 items-center justify-center rounded-sm bg-primary font-display text-lg text-on-primary">
                    "D"
                </div>
                <div>
                    <p class="font-display text-xl leading-none text-on-surface">
                        "Dainiki"
                    </p>
                    <p class="mt-1 font-ui text-ui-label uppercase tracking-[0.14em] text-on-surface-variant">
                        "A quiet journal"
                    </p>
                </div>
            </div>

            <nav class="mt-10 space-y-1">
                <a
                    href="/"
                    class="flex items-center justify-between rounded-sm bg-surface-container px-3 py-2 font-ui text-ui-body text-on-surface"
                >
                    <span>"Timeline"</span>
                    <span class="h-1.5 w-1.5 rounded-full bg-primary"></span>
                </a>
                <button
                    type="button"
                    class="flex w-full items-center justify-between rounded-sm px-3 py-2 text-left font-ui text-ui-body text-on-surface-variant hover:bg-surface-container hover:text-on-surface"
                >
                    <span>"Search"</span>
                    <span class="font-ui text-ui-label text-on-surface-variant">
                        "⌘K"
                    </span>
                </button>
            </nav>

            <div class="mt-8">
                <h2 class="px-3 font-ui text-ui-label uppercase tracking-[0.14em] text-on-surface-variant">
                    "Tags"
                </h2>
                <div class="mt-2 space-y-0.5">
                    for tag in ["#morning", "#reflections", "#work", "#ideas"] {
                        <button
                            type="button"
                            class="flex w-full items-center justify-between rounded-sm px-3 py-1.5 text-left font-ui text-ui-body text-on-surface-variant hover:bg-surface-container hover:text-on-surface"
                        >
                            <span>(tag)</span>
                            <span class="font-ui text-ui-label text-on-surface-variant/70">
                                "·"
                            </span>
                        </button>
                    }
                </div>
            </div>

            <div class="mt-auto border-t border-outline-variant/60 pt-4">
                <button
                    type="button"
                    class="flex w-full items-center gap-2 rounded-sm px-3 py-2 text-left font-ui text-ui-body text-on-surface-variant hover:bg-surface-container hover:text-on-surface"
                >
                    <span aria-hidden="true">"⚙"</span>
                    <span>"Settings"</span>
                </button>
                <p class="px-3 pt-3 font-ui text-ui-label text-on-surface-variant/70">
                    "Saved on this device"
                </p>
            </div>
        </aside>
    })
}

#[component]
pub async fn topbar() -> Result<impl View> {
    Ok(view! {
        <header class="flex h-16 items-center justify-between gap-4 border-b border-outline-variant/70 px-6">
            <button
                type="button"
                class="flex min-w-0 max-w-md flex-1 items-center gap-3 rounded-sm border border-outline-variant/70 bg-surface-container-lowest px-3 py-2 text-left font-ui text-ui-body text-on-surface-variant hover:border-outline"
            >
                <span>"Search entries…"</span>
                <span class="ml-auto font-ui text-ui-label text-on-surface-variant/70">
                    "⌘K"
                </span>
            </button>

            <div class="flex shrink-0 items-center gap-2 font-ui text-ui-label text-on-surface-variant">
                <span class="h-1.5 w-1.5 rounded-full bg-primary"></span>
                "Local mode"
            </div>
        </header>
    })
}
