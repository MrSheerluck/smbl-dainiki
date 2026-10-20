use topcoat::{
    Result,
    view::{View, component, view},
};

#[component]
pub async fn pull_quote() -> Result<impl View> {
    Ok(view! {
        <section
            class="max-w-4xl mx-auto px-6 lg:px-12 py-16 sm:py-20 text-center scroll-mt-24"
            id="manifesto"
        >
            <div class="space-y-6">
                <div
                    class="font-display-date text-display-date text-primary/30 leading-none select-none"
                >
                    "“"
                </div>
                <p
                    class="font-headline-lg text-[2rem] leading-[1.25] sm:text-[2.35rem] sm:leading-[1.25] text-on-surface italic max-w-3xl mx-auto text-balance"
                >
                    "“Meaningful reflection does not emerge from forcing friction, but rather from aligning oneself with the grain of your own memory.”"
                </p>
                <div class="flex flex-col items-center gap-1">
                    <span class="font-ui-header text-ui-header text-on-surface">
                        "The SMBL Journaling Treatise"
                    </span>
                    <span
                        class="font-ui-caption text-ui-caption text-on-surface-variant"
                    >
                        "Kyoto · Volume IV"
                    </span>
                </div>
                <div class="pt-2"><div class="w-16 h-px rule-fade mx-auto"></div></div>
            </div>
        </section>
    })
}
