use topcoat::{
    Result,
    view::{View, component, view},
};

#[component]
pub async fn hero() -> Result<impl View> {
    Ok(view! {
        <section
            class="relative w-full overflow-hidden pt-28 pb-16 sm:pt-36 sm:pb-20"
            id="top"
        >
            <div class="absolute inset-0 paper-grain pointer-events-none -z-10"></div>

            <div
                class="max-w-5xl mx-auto px-6 lg:px-12 text-center flex flex-col items-center"
            >
                <p
                    class="flex items-center gap-3 font-ui-caption text-ui-caption uppercase tracking-[0.18em] text-primary/75"
                >
                    <span class="h-px w-7 bg-primary/35"></span>
                    "Now open · Local-first journal"
                    <span class="h-px w-7 bg-primary/35"></span>
                </p>

                <h1
                    class="font-headline-lg text-[2.6rem] leading-[1.08] sm:text-[3.65rem] sm:leading-[1.04] tracking-tight text-on-surface max-w-4xl text-balance mt-7"
                >
                    "A quiet, continuous river "
                    <br class="hidden sm:inline">
                    "for your thoughts."
                </h1>

                <p
                    class="font-journal-body-lg text-journal-body-lg text-on-surface-variant max-w-2xl mt-7 leading-relaxed text-pretty"
                >
                    "A private, local-first daily journal for the web. Plain Markdown, a calm timeline, tags, search, and a calendar. Yours without an account. Sync only when you want it."
                </p>

                <div
                    class="mt-9 flex flex-col sm:flex-row items-center justify-center gap-3"
                >
                    <a
                        class="inline-flex items-center justify-center px-5 py-2.5 rounded bg-primary text-on-primary font-ui-label text-ui-label hover:bg-primary-container hover:text-on-primary-container transition-colors"
                        href="https://app.dainiki.smbl.dev/"
                    >
                        "Open your journal"
                    </a>
                    <a
                        class="inline-flex items-center justify-center px-5 py-2.5 rounded border border-outline-variant text-on-surface font-ui-label text-ui-label hover:bg-surface-container transition-colors"
                        href="#pricing"
                    >
                        "See sync plans"
                    </a>
                </div>

                <p
                    class="mt-5 font-ui-caption text-ui-caption text-on-surface-variant/70"
                >
                    "Local journaling is free. Sync is $2/month or $20/year."
                </p>
            </div>
        </section>
    })
}
