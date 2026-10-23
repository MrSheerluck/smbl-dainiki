use topcoat::{
    Result,
    view::{View, component, view},
};

#[component]
pub async fn faq() -> Result<impl View> {
    Ok(view! {
        <section
            class="max-w-3xl mx-auto px-6 lg:px-12 py-20 sm:py-24 scroll-mt-24"
            id="faq"
        >
            <div class="text-center mb-10">
                <span
                    class="font-ui-caption text-ui-caption text-primary uppercase tracking-widest font-semibold"
                >
                    "FAQ"
                </span>
                <h2
                    class="font-headline-lg text-headline-lg text-on-surface mt-2 text-balance"
                >
                    "A few honest answers."
                </h2>
            </div>

            <div
                class="divide-y divide-outline-variant/30 border-y border-outline-variant/30"
            >
                <details class="py-5">
                    <summary
                        class="flex items-center justify-between gap-4 cursor-pointer list-none"
                    >
                        <span class="font-headline-sm text-headline-sm text-on-surface">
                            "Do I need an account?"
                        </span>
                        <span
                            class="faq-plus text-xl font-light text-on-surface-variant leading-none"
                        >
                            "+"
                        </span>
                    </summary>
                    <p
                        class="font-journal-body text-journal-body text-on-surface-variant leading-relaxed mt-3 pr-8"
                    >
                        "No. The local journal opens without an account. An account is only needed for paid sync."
                    </p>
                </details>

                <details class="py-5">
                    <summary
                        class="flex items-center justify-between gap-4 cursor-pointer list-none"
                    >
                        <span class="font-headline-sm text-headline-sm text-on-surface">
                            "Where is my journal stored?"
                        </span>
                        <span
                            class="faq-plus text-xl font-light text-on-surface-variant leading-none"
                        >
                            "+"
                        </span>
                    </summary>
                    <p
                        class="font-journal-body text-journal-body text-on-surface-variant leading-relaxed mt-3 pr-8"
                    >
                        "In your browser’s local storage by default, or in a folder you connect on Chromium browsers. Either way, every entry is a plain Markdown file you can export at any time."
                    </p>
                </details>

                <details class="py-5">
                    <summary
                        class="flex items-center justify-between gap-4 cursor-pointer list-none"
                    >
                        <span class="font-headline-sm text-headline-sm text-on-surface">
                            "Can I add images, audio, or video?"
                        </span>
                        <span
                            class="faq-plus text-xl font-light text-on-surface-variant leading-none"
                        >
                            "+"
                        </span>
                    </summary>
                    <p
                        class="font-journal-body text-journal-body text-on-surface-variant leading-relaxed mt-3 pr-8"
                    >
                        "Dainiki is text-only by design. Just words."
                    </p>
                </details>

                <details class="py-5">
                    <summary
                        class="flex items-center justify-between gap-4 cursor-pointer list-none"
                    >
                        <span class="font-headline-sm text-headline-sm text-on-surface">
                            "Is there a free trial?"
                        </span>
                        <span
                            class="faq-plus text-xl font-light text-on-surface-variant leading-none"
                        >
                            "+"
                        </span>
                    </summary>
                    <p
                        class="font-journal-body text-journal-body text-on-surface-variant leading-relaxed mt-3 pr-8"
                    >
                        "No. Local journaling is free forever. Sync is a paid feature from the first day: $2/month or $20/year. That keeps the product honest and the storage costs predictable."
                    </p>
                </details>

                <details class="py-5">
                    <summary
                        class="flex items-center justify-between gap-4 cursor-pointer list-none"
                    >
                        <span class="font-headline-sm text-headline-sm text-on-surface">
                            "What happens if I cancel sync?"
                        </span>
                        <span
                            class="faq-plus text-xl font-light text-on-surface-variant leading-none"
                        >
                            "+"
                        </span>
                    </summary>
                    <p
                        class="font-journal-body text-journal-body text-on-surface-variant leading-relaxed mt-3 pr-8"
                    >
                        "Your local journal stays exactly where it is. Sync pauses. The encrypted server copy becomes read-only for 30 days, then it is deleted. You can export before or after cancelling."
                    </p>
                </details>

                <details class="py-5">
                    <summary
                        class="flex items-center justify-between gap-4 cursor-pointer list-none"
                    >
                        <span class="font-headline-sm text-headline-sm text-on-surface">
                            "How much sync storage do I get?"
                        </span>
                        <span
                            class="faq-plus text-xl font-light text-on-surface-variant leading-none"
                        >
                            "+"
                        </span>
                    </summary>
                    <p
                        class="font-journal-body text-journal-body text-on-surface-variant leading-relaxed mt-3 pr-8"
                    >
                        "1 GB per paid account. Because Dainiki is text-only, that is roughly 200,000 typical entries. Most journals will never come close."
                    </p>
                </details>

                <details class="py-5">
                    <summary
                        class="flex items-center justify-between gap-4 cursor-pointer list-none"
                    >
                        <span class="font-headline-sm text-headline-sm text-on-surface">
                            "Can I use Obsidian with it?"
                        </span>
                        <span
                            class="faq-plus text-xl font-light text-on-surface-variant leading-none"
                        >
                            "+"
                        </span>
                    </summary>
                    <p
                        class="font-journal-body text-journal-body text-on-surface-variant leading-relaxed mt-3 pr-8"
                    >
                        "Yes. Connect a folder in Chrome or Edge and point Obsidian at the same folder. The daily files are ordinary Markdown."
                    </p>
                </details>

                <details class="py-5">
                    <summary
                        class="flex items-center justify-between gap-4 cursor-pointer list-none"
                    >
                        <span class="font-headline-sm text-headline-sm text-on-surface">
                            "Can I self-host it?"
                        </span>
                        <span
                            class="faq-plus text-xl font-light text-on-surface-variant leading-none"
                        >
                            "+"
                        </span>
                    </summary>
                    <p
                        class="font-journal-body text-journal-body text-on-surface-variant leading-relaxed mt-3 pr-8"
                    >
                        "Yes. Dainiki is MIT licensed. The self-hosted build includes auth, sync, and storage configuration, with SMTP or Resend for email and Cloudflare R2 or a local disk for storage."
                    </p>
                </details>
            </div>

            <p
                class="mt-8 text-center font-ui-caption text-ui-caption text-on-surface-variant"
            >
                "Still curious? "
                <a
                    class="text-primary hover:text-on-surface underline underline-offset-4 transition-colors"
                    href="mailto:hello@smbl.dev"
                >
                    "Write to us."
                </a>
            </p>
        </section>
    })
}
