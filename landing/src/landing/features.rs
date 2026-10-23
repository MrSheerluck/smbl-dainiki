use topcoat::{
    Result,
    view::{View, component, view},
};

struct Feature {
    title: &'static str,
    body: &'static str,
}

const FEATURES: &[Feature] = &[
    Feature {
        title: "Text-only by design",
        body: "Dainiki is built around the page. Words, paragraphs, and the quiet accumulation of days.",
    },
    Feature {
        title: "Plain Markdown files",
        body: "Every day is a readable .md file in a folder you choose. Open it in Obsidian, Git, or any text editor.",
    },
    Feature {
        title: "Local-first",
        body: "Your browser holds the journal. Write offline, reload, open the same files in another editor. The journal is yours to keep.",
    },
    Feature {
        title: "Optional encrypted sync",
        body: "$2/month or $20/year for multi-device sync. End-to-end encrypted, 1 GB, and easy to cancel.",
    },
];

#[component]
pub async fn features() -> Result<impl View> {
    Ok(view! {
        <section
            class="w-full bg-surface-container-low/60 border-y border-surface-container-high/50 scroll-mt-24"
            id="features"
        >
            <div class="max-w-6xl mx-auto px-6 lg:px-12 py-20 sm:py-24">
                <div class="max-w-2xl">
                    <span
                        class="font-ui-caption text-ui-caption text-primary uppercase tracking-widest font-semibold"
                    >
                        "Why Dainiki"
                    </span>
                    <h2
                        class="font-headline-lg text-headline-lg text-on-surface mt-2 text-balance"
                    >
                        "A journal that stays out of the way."
                    </h2>
                    <p
                        class="font-journal-body-lg text-journal-body-lg text-on-surface-variant mt-4 leading-relaxed"
                    >
                        "A quiet surface for the practice of paying attention. One continuous stream of days, built for attention rather than management."
                    </p>
                </div>

                <div class="grid grid-cols-1 md:grid-cols-2 gap-5 mt-12">
                    for feature in FEATURES {
                        <div
                            class="p-6 sm:p-7 rounded-xl bg-surface-container-lowest border border-surface-container-high/50 space-y-4"
                        >
                            <span class="block h-px w-8 bg-primary/30"></span>
                            <h3
                                class="font-headline-md text-headline-md text-on-surface"
                            >
                                (feature.title)
                            </h3>
                            <p
                                class="font-journal-body text-journal-body text-on-surface-variant leading-relaxed"
                            >
                                (feature.body)
                            </p>
                        </div>
                    }
                </div>
            </div>
        </section>
    })
}
