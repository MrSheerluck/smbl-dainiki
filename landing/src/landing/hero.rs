use topcoat::{
    Result,
    router::href,
    view::{View, class, component, view},
};

use crate::{landing::WaitlistStatus, waitlist::join};

#[component]
pub async fn hero(status: Option<WaitlistStatus>) -> Result<impl View> {
    let (message, tone) = match status {
        Some(status) => (status.message(), status.tone_class()),
        None => ("", ""),
    };

    Ok(view! {
        <section
            class="relative w-full overflow-hidden pt-28 pb-16 sm:pt-36 sm:pb-20"
            id="top"
        >
            <div
                class="absolute -top-40 left-1/2 -translate-x-1/2 w-[900px] h-[420px] bg-gradient-to-b from-primary-fixed/25 via-surface-container-high/40 to-transparent blur-3xl pointer-events-none -z-10"
            ></div>
            <div class="absolute inset-0 paper-grain pointer-events-none -z-10"></div>

            <div
                class="max-w-5xl mx-auto px-6 lg:px-12 text-center flex flex-col items-center"
            >
                <div
                    class="inline-flex flex-wrap items-center justify-center gap-2 px-3.5 py-1.5 rounded-full bg-surface-container-high/80 text-on-surface-variant font-ui-caption text-ui-caption shadow-sm mb-8"
                >
                    <span class="w-1.5 h-1.5 rounded-full bg-primary animate-pulse"></span>
                    <span class="font-ui-label text-ui-label tracking-wide uppercase">
                        "Open source"
                    </span>
                    <span class="text-outline-variant">"·"</span>
                    <span>"Local-first"</span>
                    <span class="text-outline-variant">"·"</span>
                    <span class="text-primary font-medium">"Text-only"</span>
                </div>

                <h1
                    class="font-headline-lg text-[2.6rem] leading-[1.08] sm:text-[3.65rem] sm:leading-[1.04] tracking-tight text-on-surface max-w-4xl text-balance"
                >
                    "A quiet, continuous river "
                    <br class="hidden sm:inline">
                    "for your thoughts."
                </h1>

                <p
                    class="font-journal-body-lg text-journal-body-lg text-on-surface-variant max-w-2xl mt-7 leading-relaxed text-pretty"
                >
                    "A private, local-first daily journal for the web. Plain Markdown, a calm timeline, tags, search, and a calendar. Sync only if you want it."
                </p>

                <div
                    class="mt-9 flex flex-col sm:flex-row items-center justify-center gap-3 w-full sm:w-auto"
                >
                    <a
                        class="inline-flex w-full sm:w-auto items-center justify-center px-6 py-3 rounded bg-primary text-on-primary font-ui-header text-ui-header shadow hover:bg-primary-container hover:text-on-primary-container transition-all"
                        href="/app"
                    >
                        "Start writing free"
                    </a>
                    <a
                        class="inline-flex w-full sm:w-auto items-center justify-center px-6 py-3 rounded border border-outline-variant/80 text-on-surface font-ui-header text-ui-header hover:bg-surface-container transition-all"
                        href="#pricing"
                    >
                        "See sync pricing"
                    </a>
                </div>

                <p
                    class="mt-5 font-ui-caption text-ui-caption text-on-surface-variant max-w-lg"
                >
                    "No account is needed for local journaling. Sync is $2/month or $20/year."
                </p>

                <div id="waitlist" class="mt-7 w-full max-w-md scroll-mt-28">
                    <form
                        class="flex flex-col sm:flex-row gap-2 items-stretch"
                        method="post"
                        action=(href!(join))
                    >
                        <input
                            class="flex-1 min-w-0 px-4 py-3 rounded bg-surface-container-lowest border border-surface-container-high text-on-surface font-ui-body placeholder:text-outline focus:outline-none focus:ring-1 focus:ring-primary shadow-sm"
                            type="email"
                            name="email"
                            required="true"
                            maxlength="254"
                            autocomplete="email"
                            inputmode="email"
                            placeholder="you@example.com"
                            aria-label="Email address"
                        >
                        <input
                            class="hidden"
                            type="text"
                            name="website"
                            tabindex="-1"
                            autocomplete="off"
                            aria-hidden="true"
                        >
                        <button
                            class="px-5 py-3 rounded border border-outline-variant bg-surface-container-lowest text-on-surface font-ui-header text-ui-header hover:bg-surface-container transition-all whitespace-nowrap"
                            type="submit"
                        >
                            "Join the waitlist"
                        </button>
                    </form>
                    if !message.is_empty() {
                        <p
                            class=(class!("mt-3 font-ui-caption text-ui-caption", tone))
                            role="status"
                            aria-live="polite"
                        >
                            (message)
                        </p>
                    }
                    <p
                        class="mt-3 font-ui-caption text-ui-caption text-on-surface-variant/70"
                    >
                        "No spam. One note when the journal opens."
                    </p>
                </div>
            </div>
        </section>
    })
}
