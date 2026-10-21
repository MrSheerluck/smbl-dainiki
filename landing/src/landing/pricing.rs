use topcoat::{
    Result,
    view::{View, component, view},
};

#[component]
pub async fn pricing() -> Result<impl View> {
    Ok(view! {
        <section
            class="w-full bg-surface-container-low py-20 sm:py-24 border-y border-surface-container-high/50 scroll-mt-24"
            id="pricing"
        >
            <div class="max-w-6xl mx-auto px-6 lg:px-12">
                <div class="max-w-2xl">
                    <span
                        class="font-ui-caption text-ui-caption text-primary uppercase tracking-widest font-semibold"
                    >
                        "Pricing"
                    </span>
                    <h2
                        class="font-headline-lg text-headline-lg text-on-surface mt-2 text-balance"
                    >
                        "Own your words. Pay only for sync."
                    </h2>
                    <p
                        class="font-journal-body-lg text-journal-body-lg text-on-surface-variant mt-4 leading-relaxed"
                    >
                        "Local journaling is free forever, with no account. Sync is optional for people who want their journal on more than one device."
                    </p>
                </div>

                <div class="grid grid-cols-1 lg:grid-cols-3 gap-5 mt-12 items-stretch">
                    <div
                        class="flex flex-col p-7 rounded-xl bg-surface-container-lowest border border-surface-container-high/60 shadow-sm"
                    >
                        <div class="flex items-center justify-between">
                            <h3
                                class="font-headline-md text-headline-md text-on-surface"
                            >
                                "Free"
                            </h3>
                            <span
                                class="font-ui-caption text-ui-caption px-2 py-0.5 rounded bg-surface-container text-on-surface-variant"
                            >
                                "Local"
                            </span>
                        </div>
                        <p
                            class="font-ui-caption text-ui-caption text-on-surface-variant mt-2"
                        >
                            "For one device and a lifetime of words."
                        </p>
                        <div class="mt-7 flex items-baseline gap-1">
                            <span
                                class="font-headline-lg text-[2.8rem] leading-none text-on-surface"
                            >
                                "$0"
                            </span>
                            <span
                                class="font-ui-caption text-ui-caption text-on-surface-variant"
                            >
                                "forever"
                            </span>
                        </div>
                        <ul
                            class="mt-7 space-y-3 font-ui-body text-ui-body text-on-surface-variant"
                        >
                            <li class="flex gap-2.5">
                                <span class="text-primary">"✓"</span>
                                "Unlimited daily entries"
                            </li>
                            <li class="flex gap-2.5">
                                <span class="text-primary">"✓"</span>
                                "Plain Markdown files"
                            </li>
                            <li class="flex gap-2.5">
                                <span class="text-primary">"✓"</span>
                                "Tags, search, calendar"
                            </li>
                            <li class="flex gap-2.5">
                                <span class="text-primary">"✓"</span>
                                "Light and dark themes"
                            </li>
                            <li class="flex gap-2.5">
                                <span class="text-primary">"✓"</span>
                                "Import and export"
                            </li>
                        </ul>
                        <div class="mt-auto pt-8">
                            <button
                                class="flex items-center justify-center w-full px-5 py-2.5 rounded border border-outline-variant text-on-surface-variant font-ui-label text-ui-label opacity-60 cursor-not-allowed"
                                type="button"
                                disabled="true"
                            >
                                "Coming soon"
                            </button>
                            <p
                                class="mt-3 text-center font-ui-caption text-ui-caption text-on-surface-variant/70"
                            >
                                "Free local journaling at launch."
                            </p>
                        </div>
                    </div>

                    <div
                        class="relative flex flex-col p-7 rounded-xl bg-surface-container-lowest border-2 border-primary/70 shadow-xl"
                    >
                        <div
                            class="absolute -top-3 left-1/2 -translate-x-1/2 px-3 py-1 rounded-full bg-primary text-on-primary font-ui-caption text-ui-caption uppercase tracking-wider"
                        >
                            "Optional"
                        </div>
                        <div class="flex items-center justify-between">
                            <h3
                                class="font-headline-md text-headline-md text-on-surface"
                            >
                                "Sync"
                            </h3>
                            <span
                                class="font-ui-caption text-ui-caption px-2 py-0.5 rounded bg-primary-fixed text-on-primary-fixed-variant font-semibold"
                            >
                                "Encrypted"
                            </span>
                        </div>
                        <p
                            class="font-ui-caption text-ui-caption text-on-surface-variant mt-2"
                        >
                            "For words that follow you between devices."
                        </p>
                        <div class="mt-7 flex items-baseline gap-1">
                            <span
                                class="font-headline-lg text-[2.8rem] leading-none text-on-surface"
                            >
                                "$2"
                            </span>
                            <span
                                class="font-ui-caption text-ui-caption text-on-surface-variant"
                            >
                                "/ month"
                            </span>
                        </div>
                        <p
                            class="mt-1 font-ui-caption text-ui-caption text-on-surface-variant"
                        >
                            "or $20/year, two months free"
                        </p>
                        <ul
                            class="mt-7 space-y-3 font-ui-body text-ui-body text-on-surface-variant"
                        >
                            <li class="flex gap-2.5">
                                <span class="text-primary">"✓"</span>
                                "End-to-end encrypted sync"
                            </li>
                            <li class="flex gap-2.5">
                                <span class="text-primary">"✓"</span>
                                "1 GB of text storage"
                            </li>
                            <li class="flex gap-2.5">
                                <span class="text-primary">"✓"</span>
                                "Conflict-safe multi-device edits"
                            </li>
                            <li class="flex gap-2.5">
                                <span class="text-primary">"✓"</span>
                                "Cancel anytime"
                            </li>
                            <li class="flex gap-2.5">
                                <span class="text-primary">"✓"</span>
                                "Billed monthly or yearly"
                            </li>
                        </ul>
                        <div class="mt-auto pt-8 space-y-2">
                            <button
                                class="flex items-center justify-center w-full px-5 py-2.5 rounded bg-primary text-on-primary font-ui-label text-ui-label opacity-60 cursor-not-allowed"
                                type="button"
                                disabled="true"
                            >
                                "Coming soon"
                            </button>
                            <p
                                class="text-center font-ui-caption text-ui-caption text-on-surface-variant/70"
                            >
                                "Checkout opens at launch. Payments by Dodo Payments."
                            </p>
                        </div>
                    </div>

                    <div
                        class="flex flex-col p-7 rounded-xl bg-surface-container-lowest border border-surface-container-high/60 shadow-sm"
                        id="self-host"
                    >
                        <div class="flex items-center justify-between">
                            <h3
                                class="font-headline-md text-headline-md text-on-surface"
                            >
                                "Self-host"
                            </h3>
                            <span
                                class="font-ui-caption text-ui-caption px-2 py-0.5 rounded bg-surface-container text-on-surface-variant"
                            >
                                "MIT"
                            </span>
                        </div>
                        <p
                            class="font-ui-caption text-ui-caption text-on-surface-variant mt-2"
                        >
                            "Run the whole thing on your own server."
                        </p>
                        <div class="mt-7 flex items-baseline gap-1">
                            <span
                                class="font-headline-lg text-[2.8rem] leading-none text-on-surface"
                            >
                                "$0"
                            </span>
                            <span
                                class="font-ui-caption text-ui-caption text-on-surface-variant"
                            >
                                "your hardware"
                            </span>
                        </div>
                        <ul
                            class="mt-7 space-y-3 font-ui-body text-ui-body text-on-surface-variant"
                        >
                            <li class="flex gap-2.5">
                                <span class="text-primary">"✓"</span>
                                "Full app and sync server"
                            </li>
                            <li class="flex gap-2.5">
                                <span class="text-primary">"✓"</span>
                                "Built-in email/password auth"
                            </li>
                            <li class="flex gap-2.5">
                                <span class="text-primary">"✓"</span>
                                "Cloudflare R2 or local disk"
                            </li>
                            <li class="flex gap-2.5">
                                <span class="text-primary">"✓"</span>
                                "Bring your own SMTP/Resend"
                            </li>
                        </ul>
                        <div class="mt-auto pt-8">
                            <button
                                class="flex items-center justify-center w-full px-5 py-2.5 rounded border border-outline-variant text-on-surface-variant font-ui-label text-ui-label opacity-60 cursor-not-allowed"
                                type="button"
                                disabled="true"
                            >
                                "Coming soon"
                            </button>
                            <p
                                class="mt-3 text-center font-ui-caption text-ui-caption text-on-surface-variant/70"
                            >
                                "The self-host guide lands with the app."
                            </p>
                        </div>
                    </div>
                </div>

                <div
                    class="mt-8 flex flex-col sm:flex-row items-start sm:items-center justify-between gap-3 text-on-surface-variant"
                >
                    <p class="font-ui-caption text-ui-caption max-w-2xl">
                        "Dodo Payments is the merchant of record for Sync and handles payment processing, taxes, invoices, and the customer portal. 1 GB is roughly 200,000 typical text entries."
                    </p>
                    <a
                        class="font-ui-caption text-ui-caption text-primary hover:text-on-surface underline underline-offset-4 whitespace-nowrap transition-colors"
                        href="mailto:hello@smbl.dev"
                    >
                        "Questions? hello@smbl.dev"
                    </a>
                </div>
            </div>
        </section>
    })
}
