use topcoat::{
    Result,
    view::{View, component, view},
};

#[component]
pub async fn markdown() -> Result<impl View> {
    Ok(view! {
        <section
            class="max-w-6xl mx-auto px-6 lg:px-12 py-20 sm:py-24 scroll-mt-24"
            id="markdown"
        >
            <div class="grid grid-cols-1 lg:grid-cols-2 gap-10 lg:gap-16 items-center">
                <div class="space-y-5">
                    <span
                        class="font-ui-caption text-ui-caption text-primary uppercase tracking-widest font-semibold"
                    >
                        "Plain files"
                    </span>
                    <h2
                        class="font-headline-lg text-headline-lg text-on-surface text-balance"
                    >
                        "Your journal is just Markdown."
                    </h2>
                    <p
                        class="font-journal-body-lg text-journal-body-lg text-on-surface-variant leading-relaxed"
                    >
                        "Your days live in a folder of text files. Open them, search them, back them up, or take them with you."
                    </p>
                    <ul
                        class="space-y-3 font-journal-body text-journal-body text-on-surface-variant"
                    >
                        <li class="flex gap-3">
                            <span class="text-primary pt-1">"✓"</span>
                            <span>
                                "One file per day: "
                                <span class="font-mono text-[0.95em] text-on-surface">
                                    "YYYY/MM/YYYY-MM-DD.md"
                                </span>
                            </span>
                        </li>
                        <li class="flex gap-3">
                            <span class="text-primary pt-1">"✓"</span>
                            <span>
                                "Works with Obsidian, Git, Dropbox, or a plain folder."
                            </span>
                        </li>
                        <li class="flex gap-3">
                            <span class="text-primary pt-1">"✓"</span>
                            <span>"Export everything from the app at any time."</span>
                        </li>
                    </ul>
                </div>

                <div
                    class="rounded-xl border border-surface-container-high/60 bg-surface-container-lowest shadow-xl overflow-hidden"
                >
                    <div
                        class="flex items-center justify-between px-4 py-2.5 border-b border-surface-container-high/60 bg-surface-container-low/60"
                    >
                        <div class="flex items-center gap-2">
                            <span class="w-2.5 h-2.5 rounded-full bg-[#ec6a5e]"></span>
                            <span class="w-2.5 h-2.5 rounded-full bg-[#f4bf4f]"></span>
                            <span class="w-2.5 h-2.5 rounded-full bg-[#62c554]"></span>
                        </div>
                        <span
                            class="font-ui-caption text-ui-caption text-on-surface-variant/70 font-mono"
                        >
                            "journal / 2026 / 10"
                        </span>
                    </div>
                    <div class="p-5 sm:p-6 space-y-5">
                        <pre
                            class="text-[12px] leading-6 font-mono text-on-surface-variant overflow-x-auto"
                        >
                            <code>
                                "journal/\n  2026/\n    10/\n      2026-10-20.md\n      2026-10-19.md\n      2026-10-18.md"
                            </code>
                        </pre>
                        <div class="h-px bg-outline-variant/30"></div>
                        <pre
                            class="text-[12px] leading-6 font-mono text-on-surface-variant overflow-x-auto"
                        >
                            <code>
                                "# The slow return of morning fog\n\n*Before the sun cleared the ridgeline...*\n\nBefore the sun cleared the ridgeline, the\nentire gorge lay suspended under a pale\nriver vapor.\n\n#morning #reflections"
                            </code>
                        </pre>
                        <div
                            class="flex items-center gap-2 pt-1 font-ui-caption text-ui-caption text-on-surface-variant/60"
                        >
                            <span class="w-1.5 h-1.5 rounded-full bg-primary"></span>
                            "Plain text, readable anywhere."
                        </div>
                    </div>
                </div>
            </div>
        </section>
    })
}
