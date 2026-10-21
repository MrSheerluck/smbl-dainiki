use topcoat::{
    Result,
    asset::asset,
    view::{View, class, component, view},
};

struct Tag {
    name: &'static str,
    count: &'static str,
}

const TAGS: &[Tag] = &[
    Tag {
        name: "reflections",
        count: "42",
    },
    Tag {
        name: "travel",
        count: "18",
    },
    Tag {
        name: "morning",
        count: "64",
    },
    Tag {
        name: "work",
        count: "31",
    },
    Tag {
        name: "ideas",
        count: "15",
    },
];

#[component]
pub async fn showcase() -> Result<impl View> {
    Ok(view! {
        <section
            class="max-w-5xl mx-auto px-4 sm:px-6 lg:px-12 pb-24 w-full scroll-mt-24"
            id="showcase"
        >
            <div class="flex flex-col items-center text-center mb-10">
                <span
                    class="font-ui-caption text-ui-caption text-primary uppercase tracking-widest font-semibold"
                >
                    "The journal"
                </span>
                <h2
                    class="font-headline-lg text-headline-lg text-on-surface mt-2 text-balance"
                >
                    "A timeline, not a dashboard."
                </h2>
                <p
                    class="font-journal-body text-journal-body text-on-surface-variant max-w-xl mt-3"
                >
                    "One continuous page. Today is always open. Older days wait quietly until you need them."
                </p>
            </div>

            <div class="rounded-xl bg-surface-container-lowest shadow-2xl p-2 sm:p-4">
                <div
                    class="flex items-center justify-between px-3.5 py-2.5 bg-surface/90 border-b border-surface-container-high/60 rounded-t-lg select-none"
                >
                    <div class="flex items-center gap-space-md">
                        <div class="flex items-center gap-1.5">
                            <div class="w-3 h-3 rounded-full bg-[#ec6a5e] shadow-inner"></div>
                            <div class="w-3 h-3 rounded-full bg-[#f4bf4f] shadow-inner"></div>
                            <div class="w-3 h-3 rounded-full bg-[#62c554] shadow-inner"></div>
                        </div>
                        <div class="flex items-center gap-space-sm pl-space-xs">
                            <img
                                alt="Dainiki logo"
                                class="h-6 w-6 rounded object-contain"
                                src=(asset!("./img/logo.png"))
                            >
                            <span
                                class="font-ui-header text-ui-header font-semibold text-on-surface tracking-tight text-xs"
                            >
                                "Dainiki"
                            </span>
                        </div>
                    </div>
                    <div class="flex items-center gap-space-sm">
                        <div
                            class="hidden sm:flex items-center gap-1.5 font-ui-caption text-ui-caption text-on-surface-variant/70"
                        >
                            <span class="w-1.5 h-1.5 rounded-full bg-primary"></span>
                            "Local · saved"
                        </div>
                        <div class="hidden sm:flex items-center">
                            <div
                                class="pl-2.5 pr-2.5 py-1 text-[11px] bg-surface-container-low rounded-full border border-surface-container text-on-surface-variant/70 w-44 flex items-center justify-between whitespace-nowrap select-none"
                            >
                                <span
                                    class="text-[11px] text-on-surface-variant/80 truncate pr-2"
                                >
                                    "Search entries..."
                                </span>
                                <span
                                    class="font-mono text-[9px] px-1.5 py-0.5 rounded bg-surface-container text-on-surface-variant/80 shrink-0 leading-none"
                                >
                                    "⌘K"
                                </span>
                            </div>
                        </div>
                    </div>
                </div>

                <div
                    class="flex flex-col md:flex-row w-full bg-surface rounded-b-lg overflow-hidden border border-t-0 border-surface-container"
                >
                    <aside
                        class="hidden md:flex flex-col justify-between w-[230px] shrink-0 bg-surface-container-low/70 border-r border-surface-container-high/50 select-none text-left"
                    >
                        <div
                            class="flex-1 flex flex-col pt-space-sm px-space-sm overflow-hidden"
                        >
                            <nav class="space-y-0.5">
                                <div
                                    class="flex items-center justify-between px-space-sm py-1 rounded bg-surface-container-highest text-on-surface font-medium text-xs"
                                >
                                    <div class="flex items-center gap-space-sm">
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
                                    </div>
                                    <span class="w-1.5 h-1.5 rounded-full bg-primary"></span>
                                </div>
                                <div
                                    class="flex items-center justify-between px-space-sm py-1 rounded text-on-surface-variant hover:bg-surface-container-high text-xs transition-colors"
                                >
                                    <div class="flex items-center gap-space-sm">
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
                                    </div>
                                    <span
                                        class="font-ui-caption text-ui-caption px-1.5 py-0.5 rounded bg-surface-container text-on-surface-variant font-mono text-[9px]"
                                    >
                                        "⌘K"
                                    </span>
                                </div>
                            </nav>

                            <div class="mt-space-md pt-space-xs">
                                <div
                                    class="px-space-sm pb-1 flex items-center justify-between text-on-surface-variant/80"
                                >
                                    <span
                                        class="font-ui-caption text-ui-caption uppercase tracking-wider font-semibold text-[10px]"
                                    >
                                        "Tags"
                                    </span>
                                    <span
                                        class="font-ui-caption text-ui-caption text-on-surface-variant/70"
                                    >
                                        "+"
                                    </span>
                                </div>
                                <nav class="space-y-0.5">
                                    for tag in TAGS {
                                        <div
                                            class="flex items-center justify-between px-space-sm py-1 rounded text-on-surface-variant font-ui-label text-xs hover:bg-surface-container-high transition-colors"
                                        >
                                            <span class="flex items-center gap-1.5">
                                                <span class="text-secondary/70 font-mono">"#"</span>
                                                (tag.name)
                                            </span>
                                            <span
                                                class="font-ui-caption text-ui-caption text-on-surface-variant/60 text-[10px]"
                                            >
                                                (tag.count)
                                            </span>
                                        </div>
                                    }
                                </nav>
                            </div>
                        </div>

                        <div
                            class="p-space-sm bg-surface-container-low/40 space-y-space-sm border-t border-surface-container-high/40"
                        >
                            <div
                                class="bg-surface-container-lowest/80 rounded-xl p-space-xs shadow-[0_1px_2px_rgba(44,42,41,0.02)]"
                            >
                                <div class="flex items-center justify-between px-1 pb-1">
                                    <span
                                        class="font-ui-caption text-ui-caption font-semibold text-on-surface text-[10px]"
                                    >
                                        "October 2026"
                                    </span>
                                    <div
                                        class="flex items-center gap-1 text-on-surface-variant"
                                    >
                                        <span class="text-[12px] leading-none">"‹"</span>
                                        <span class="text-[12px] leading-none">"›"</span>
                                    </div>
                                </div>
                                <div
                                    class="grid grid-cols-7 gap-1 text-center font-ui-caption text-ui-caption text-on-surface-variant text-[9px]"
                                >
                                    <span>"M"</span>
                                    <span>"T"</span>
                                    <span>"W"</span>
                                    <span>"T"</span>
                                    <span>"F"</span>
                                    <span>"S"</span>
                                    <span>"S"</span>
                                </div>
                                <div
                                    class="grid grid-cols-7 gap-1 text-center font-ui-caption text-ui-caption pt-1 text-[10px]"
                                >
                                    <span class="text-on-surface-variant/30">"28"</span>
                                    <span class="text-on-surface-variant/30">"29"</span>
                                    <span class="text-on-surface-variant/30">"30"</span>
                                    for day in 1..=31u8 {
                                        <span
                                            class=(class!(
                                                "text-on-surface-variant" if day != 20,
                                                "font-semibold text-primary relative after:content-[''] after:absolute after:bottom-0 after:left-1/2 after:-translate-x-1/2 after:w-1 after:h-1 after:rounded-full after:bg-primary" if day
                                                    == 20,
                                            ))
                                        >
                                            (day)
                                        </span>
                                    }
                                </div>
                            </div>
                            <div
                                class="flex items-center justify-between px-space-xs text-xs"
                            >
                                <div
                                    class="flex items-center gap-space-sm text-on-surface-variant font-ui-label"
                                >
                                    <span class="text-[14px] leading-none">"⚙"</span>
                                    <span>"Settings"</span>
                                </div>
                                <span
                                    class="font-ui-caption text-ui-caption px-1.5 py-0.5 rounded bg-surface-container text-on-surface-variant/70 font-mono text-[9px]"
                                >
                                    "⌘,"
                                </span>
                            </div>
                            <div
                                class="flex items-center gap-1.5 px-space-xs text-on-surface-variant/60"
                            >
                                <span class="w-1.5 h-1.5 rounded-full bg-primary"></span>
                                <span class="font-ui-caption text-ui-caption text-[10px]">
                                    "Stored on this device"
                                </span>
                            </div>
                        </div>
                    </aside>

                    <div
                        class="flex-1 bg-surface p-4 sm:p-6 lg:p-8 overflow-hidden text-left relative"
                    >
                        <div class="max-w-[44rem] mx-auto">
                            <article class="flex gap-3 sm:gap-4 mb-7">
                                <div class="w-11 sm:w-14 shrink-0 pt-1 text-right">
                                    <span
                                        class="block font-display-date text-[1.9rem] sm:text-[2.2rem] leading-none text-primary"
                                    >
                                        "20"
                                    </span>
                                    <span
                                        class="mt-1 block font-ui-caption text-[8px] sm:text-[9px] uppercase tracking-wide text-primary"
                                    >
                                        "Today"
                                    </span>
                                    <span
                                        class="mt-0.5 block font-ui-caption text-[8px] sm:text-[9px] text-on-surface-variant/50"
                                    >
                                        "Oct 2026"
                                    </span>
                                </div>
                                <div
                                    class="relative flex-1 border-l border-outline-variant/25 pl-3 sm:pl-4"
                                >
                                    <span
                                        class="absolute -left-1 top-7 h-1.5 w-1.5 rounded-full bg-primary/60"
                                    ></span>
                                    <div
                                        class="rounded-xl border border-surface-container-high/50 bg-surface-container-lowest/90 p-4 sm:p-5 shadow-[0_1px_3px_rgba(44,42,41,0.04)]"
                                    >
                                        <div
                                            class="flex items-baseline justify-between gap-3 mb-3"
                                        >
                                            <h3
                                                class="font-headline-md text-headline-md text-on-surface tracking-tight font-normal"
                                            >
                                                "The slow return of morning fog"
                                            </h3>
                                            <span
                                                class="hidden sm:inline font-ui-caption text-ui-caption text-on-surface-variant/40 whitespace-nowrap"
                                            >
                                                "Saved just now"
                                            </span>
                                        </div>
                                        <div
                                            class="space-y-space-sm font-journal-body text-[0.95rem] text-on-surface leading-relaxed"
                                        >
                                            <p
                                                class="first-letter:font-journal-body-lg first-letter:text-[2.2rem] first-letter:leading-none first-letter:float-left first-letter:mr-2 first-letter:font-normal first-letter:text-primary"
                                            >
                                                "Before the sun cleared the ridgeline, the entire gorge lay suspended under a pale river vapor. Walking the bamboo path at this hour is an exercise in acoustic subtraction: only the steady, hollow knocking of towering culms overhead. I stopped for roasted tea by the hearth, grateful for an unhurried morning."
                                            </p>
                                            <blockquote
                                                class="border-l-2 border-secondary/40 pl-4 py-1 my-4 font-journal-quote text-journal-quote text-on-surface-variant italic"
                                            >
                                                "The day did not ask to be optimized. It asked to be noticed."
                                            </blockquote>
                                            <p>
                                                "A few lines before the inbox opens. A small record of what the light was doing, and what I was thinking while it did it."
                                            </p>
                                            <div class="pt-1 flex items-center justify-between gap-3">
                                                <div
                                                    class="flex items-center gap-2 font-ui-caption text-ui-caption text-on-surface-variant/60"
                                                >
                                                    <span class="text-secondary/70 font-mono">"#morning"</span>
                                                    <span class="text-secondary/70 font-mono">
                                                        "#reflections"
                                                    </span>
                                                </div>
                                            </div>
                                            <div
                                                class="pt-1 text-on-surface-variant/40 flex items-center gap-1.5"
                                            >
                                                <span
                                                    class="w-0.5 h-4 bg-primary/70 animate-pulse inline-block"
                                                ></span>
                                                <span class="italic font-journal-body text-xs">
                                                    "Add thoughts for today..."
                                                </span>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </article>

                            <article class="flex gap-3 sm:gap-4 mb-7">
                                <div class="w-11 sm:w-14 shrink-0 pt-1 text-right">
                                    <span
                                        class="block font-display-date text-[1.9rem] sm:text-[2.2rem] leading-none text-on-surface/70"
                                    >
                                        "19"
                                    </span>
                                    <span
                                        class="mt-1 block font-ui-caption text-[8px] sm:text-[9px] uppercase tracking-wide text-on-surface-variant/70"
                                    >
                                        "Monday"
                                    </span>
                                    <span
                                        class="mt-0.5 block font-ui-caption text-[8px] sm:text-[9px] text-on-surface-variant/50"
                                    >
                                        "Oct 2026"
                                    </span>
                                </div>
                                <div
                                    class="relative flex-1 border-l border-outline-variant/25 pl-3 sm:pl-4"
                                >
                                    <span
                                        class="absolute -left-1 top-7 h-1.5 w-1.5 rounded-full bg-outline-variant"
                                    ></span>
                                    <div
                                        class="rounded-xl border border-surface-container-high/50 bg-surface-container-lowest/70 p-4 sm:p-5"
                                    >
                                        <h3
                                            class="font-headline-md text-headline-md text-on-surface tracking-tight font-normal mb-3"
                                        >
                                            "A small room for attention"
                                        </h3>
                                        <div
                                            class="space-y-space-sm font-journal-body text-[0.95rem] text-on-surface leading-relaxed"
                                        >
                                            <p>
                                                "Spent the morning clearing the desk instead of clearing the inbox. Two notebooks, one pen, a glass of water. The blank page stopped feeling like a debt and started feeling like a room."
                                            </p>
                                            <blockquote
                                                class="border-l-2 border-secondary/40 pl-4 py-1 my-4 font-journal-quote text-journal-quote text-on-surface-variant italic"
                                            >
                                                "Attention is a place you can return to."
                                            </blockquote>
                                            <div
                                                class="flex items-center gap-2 font-ui-caption text-ui-caption text-on-surface-variant/60"
                                            >
                                                <span class="text-secondary/70 font-mono">"#work"</span>
                                                <span class="text-secondary/70 font-mono">"#morning"</span>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </article>

                            <article class="flex gap-3 sm:gap-4">
                                <div class="w-11 sm:w-14 shrink-0 pt-1 text-right">
                                    <span
                                        class="block font-display-date text-[1.9rem] sm:text-[2.2rem] leading-none text-on-surface/70"
                                    >
                                        "18"
                                    </span>
                                    <span
                                        class="mt-1 block font-ui-caption text-[8px] sm:text-[9px] uppercase tracking-wide text-on-surface-variant/70"
                                    >
                                        "Sunday"
                                    </span>
                                    <span
                                        class="mt-0.5 block font-ui-caption text-[8px] sm:text-[9px] text-on-surface-variant/50"
                                    >
                                        "Oct 2026"
                                    </span>
                                </div>
                                <div
                                    class="relative flex-1 border-l border-outline-variant/25 pl-3 sm:pl-4"
                                >
                                    <span
                                        class="absolute -left-1 top-7 h-1.5 w-1.5 rounded-full bg-outline-variant"
                                    ></span>
                                    <div
                                        class="rounded-xl border border-surface-container-high/50 bg-surface-container-lowest/70 p-4 sm:p-5"
                                    >
                                        <div
                                            class="space-y-space-sm font-journal-body text-[0.95rem] text-on-surface leading-relaxed"
                                        >
                                            <p>
                                                "Late afternoon sunlight cut cleanly across the white oak table. I left the phone in the other room and wrote until the room turned amber. Nothing important happened. That was the point."
                                            </p>
                                            <div
                                                class="flex items-center gap-2 font-ui-caption text-ui-caption text-on-surface-variant/60"
                                            >
                                                <span class="text-secondary/70 font-mono">
                                                    "#reflections"
                                                </span>
                                            </div>
                                        </div>
                                    </div>
                                </div>
                            </article>

                            <div
                                class="pt-space-md pb-space-xs flex flex-col items-center justify-center text-center opacity-60"
                            >
                                <span class="text-[14px] text-on-surface-variant mb-0.5">
                                    "↓"
                                </span>
                                <span
                                    class="font-ui-caption text-ui-caption text-on-surface-variant tracking-wider uppercase text-[9px]"
                                >
                                    "October 17, 2026 & earlier"
                                </span>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </section>
    })
}
