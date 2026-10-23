use std::{
    collections::{BTreeMap, BTreeSet},
    fmt::Write,
};

use dainiki_core::{Date, EntryMeta, month_short, month_year, weekday_name};
use time::Month;

pub struct EntryRecord {
    pub iso: String,
    pub text: String,
    pub meta: EntryMeta,
}

pub fn timeline(
    entries: &[EntryRecord],
    selected_iso: &str,
    active_tag: Option<&str>,
    limit: usize,
) -> String {
    let mut html = String::new();
    let visible: Vec<&EntryRecord> = entries
        .iter()
        .filter(|entry| {
            entry.iso != selected_iso
                && active_tag.is_none_or(|tag| entry.meta.tags.iter().any(|value| value == tag))
        })
        .collect();
    let total = visible.len();

    for entry in visible.iter().take(limit) {
        let date = entry.meta.date;
        let excerpt = escape_html(if entry.meta.excerpt.is_empty() {
            "Empty entry"
        } else {
            &entry.meta.excerpt
        });

        let mut tags = String::new();
        for tag in &entry.meta.tags {
            let _ = write!(
                tags,
                r#"<span class="font-mono text-ui-label text-secondary/70">#{}</span>"#,
                escape_html(tag)
            );
        }

        let _ = write!(
            html,
            r##"<article class="flex gap-3 sm:gap-5" data-entry-iso="{iso}">
  <div class="w-14 shrink-0 pt-1 text-right">
    <span class="block font-display text-[1.9rem] leading-none text-on-surface/70 sm:text-[2.2rem]">{day}</span>
    <span class="mt-1 block whitespace-nowrap font-ui text-[8px] uppercase tracking-wide text-on-surface-variant/70 sm:text-[9px]">{weekday}</span>
    <span class="mt-0.5 block font-ui text-[8px] text-on-surface-variant/50 sm:text-[9px]">{month}</span>
  </div>
  <div class="relative min-w-0 flex-1 border-l border-outline-variant/25 pl-4 sm:pl-6">
    <span class="absolute -left-1 top-7 h-1.5 w-1.5 rounded-full bg-outline-variant"></span>
    <a href="#entry-editor" data-iso="{iso}" aria-label="Open the entry for {weekday}, {month} {day}" class="block rounded-xl border border-outline-variant/40 bg-surface-container-lowest/80 p-5 shadow-sm transition-colors hover:border-outline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/40 sm:p-6">
      <p class="font-journal text-journal text-on-surface">{excerpt}</p>
      <div class="mt-4 flex flex-wrap items-center justify-between gap-3">
        <div class="flex flex-wrap items-center gap-3">{tags}</div>
        <span class="font-ui text-[10px] text-on-surface-variant/50">{words} words</span>
      </div>
    </a>
  </div>
</article>"##,
            day = date.day(),
            weekday = weekday_name(date),
            month = month_year(date),
            iso = escape_html(&entry.iso),
            excerpt = excerpt,
            tags = tags,
            words = entry.meta.words,
        );
    }

    if html.is_empty() {
        let message = active_tag.map_or_else(
            || "No older entries yet.".to_string(),
            |tag| format!("No entries tagged #{}.", escape_html(tag)),
        );

        let _ = write!(
            html,
            r#"<p class="font-ui text-ui-label text-on-surface-variant/60">{message}</p>"#
        );
    } else if total > limit {
        let _ = write!(
            html,
            r#"<div class="pt-2 text-center">
  <button type="button" data-load-older="true" class="rounded border border-outline-variant/60 px-4 py-2 font-ui text-ui-body text-on-surface-variant hover:bg-surface-container hover:text-on-surface">
    Load older entries
  </button>
  <p class="mt-2 font-ui text-[10px] text-on-surface-variant/50">Showing {shown} of {total}</p>
</div>"#,
            shown = limit.min(total),
            total = total
        );
    }

    html
}

pub fn search_results(
    entries: &[EntryRecord],
    query: &str,
    selected: usize,
) -> (String, Vec<String>, usize) {
    let query = query.trim().to_lowercase();
    let matches: Vec<&EntryRecord> = entries
        .iter()
        .filter(|entry| {
            query.is_empty()
                || entry.text.to_lowercase().contains(&query)
                || entry.meta.tags.iter().any(|tag| tag.contains(&query))
        })
        .take(20)
        .collect();

    if matches.is_empty() {
        return (
            r#"<p class="px-5 py-6 font-ui text-ui-label text-on-surface-variant/60">No results.</p>"#
                .to_string(),
            Vec::new(),
            0,
        );
    }

    let selected = selected.min(matches.len() - 1);
    let mut html = String::new();
    let mut isos = Vec::new();

    for (index, entry) in matches.iter().enumerate() {
        let date = entry.meta.date;
        let selected_class = if index == selected {
            " bg-surface-container-high"
        } else {
            ""
        };

        let mut tags = String::new();
        for tag in &entry.meta.tags {
            let _ = write!(
                tags,
                r#"<span class="font-mono text-[10px] text-secondary/70">#{}</span>"#,
                escape_html(tag)
            );
        }

        let _ = write!(
            html,
            r##"<button type="button" data-iso="{iso}" aria-selected="{aria}" class="block w-full border-b border-outline-variant/30 px-5 py-3 text-left last:border-0 hover:bg-surface-container{selected_class}">
  <div class="flex items-baseline justify-between gap-3">
    <span class="font-display text-sm text-on-surface">{date_label}</span>
    <span class="font-ui text-[10px] text-on-surface-variant/50">{words} words</span>
  </div>
  <p class="mt-1 font-journal text-journal text-on-surface-variant">{snippet}</p>
  <div class="mt-2 flex flex-wrap gap-2">{tags}</div>
</button>"##,
            iso = escape_html(&entry.iso),
            aria = if index == selected { "true" } else { "false" },
            selected_class = selected_class,
            date_label = format!(
                "{}, {} {}, {}",
                weekday_name(date),
                month_short(date),
                date.day(),
                date.year()
            ),
            words = entry.meta.words,
            snippet = escape_html(&search_snippet(entry, query.as_str())),
            tags = tags,
        );

        isos.push(entry.iso.clone());
    }

    (html, isos, selected)
}

fn search_snippet(entry: &EntryRecord, query: &str) -> String {
    if query.is_empty() {
        return entry.meta.excerpt.clone();
    }

    let lower = entry.text.to_lowercase();
    let Some(position) = lower.find(query) else {
        return entry.meta.excerpt.clone();
    };

    let mut start = position.saturating_sub(40);

    while start > 0 && !entry.text.is_char_boundary(start) {
        start -= 1;
    }

    let mut end = (position + query.len() + 80).min(entry.text.len());

    while end < entry.text.len() && !entry.text.is_char_boundary(end) {
        end += 1;
    }

    let mut snippet = entry.text[start..end].replace('\n', " ");

    if start > 0 {
        snippet.insert(0, '…');
    }

    if end < entry.text.len() {
        snippet.push('…');
    }

    snippet
}

pub fn inline_editor() -> String {
    r#"<div class="rounded-xl border border-outline-variant/40 bg-surface-container-lowest/80 p-5 shadow-sm sm:p-6">
  <div class="inline-live live-editor min-h-[10rem] w-full font-journal text-journal text-on-surface outline-none" contenteditable="true" spellcheck="true" role="textbox" aria-multiline="true" aria-label="Edit entry"></div>
  <div class="mt-3 flex items-center justify-between gap-3 border-t border-outline-variant/30 pt-2">
    <span aria-live="polite" class="inline-status font-ui text-[10px] text-on-surface-variant/60">Editing</span>
    <button type="button" class="inline-done rounded bg-primary px-3 py-1 font-ui text-ui-label text-on-primary hover:bg-primary-container hover:text-on-primary-container">
      Done
    </button>
  </div>
</div>"#
        .to_string()
}

pub fn editor_tags(tags: &[String]) -> String {
    tags.iter()
        .map(|tag| {
            format!(
                r#"<span class="font-mono text-ui-label text-secondary/70">#{}</span>"#,
                escape_html(tag)
            )
        })
        .collect()
}

pub fn tags(entries: &[EntryRecord], active_tag: Option<&str>) -> String {
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();

    for entry in entries {
        for tag in &entry.meta.tags {
            *counts.entry(tag.clone()).or_default() += 1;
        }
    }

    let mut tags: Vec<(String, usize)> = counts.into_iter().collect();
    tags.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    tags.truncate(8);

    if tags.is_empty() {
        return r#"<p class="px-2 py-1 font-ui text-[10px] text-on-surface-variant/50">No tags yet.</p>"#
            .to_string();
    }

    let mut html = String::new();

    for (tag, count) in tags {
        let class = if active_tag == Some(tag.as_str()) {
            "flex w-full items-center justify-between rounded bg-surface-container-high px-2 py-1 text-left font-ui text-ui-label text-on-surface"
        } else {
            "flex w-full items-center justify-between rounded px-2 py-1 text-left font-ui text-ui-label text-on-surface-variant transition-colors hover:bg-surface-container-high hover:text-on-surface"
        };

        let _ = write!(
            html,
            r#"<button type="button" data-tag="{tag}" class="{class}">
  <span class="flex items-center gap-1.5">
    <span class="font-mono text-secondary/70">#</span>
    <span>{label}</span>
  </span>
  <span class="font-ui text-[10px] text-on-surface-variant/60">{count}</span>
</button>"#,
            tag = escape_html(&tag),
            label = escape_html(&tag),
            class = class,
            count = count,
        );
    }

    html
}

pub fn calendar(
    year: i32,
    month: u32,
    selected_iso: &str,
    today_iso: &str,
    entries: &[EntryRecord],
) -> String {
    let Some(month_value) = Month::try_from(month as u8).ok() else {
        return String::new();
    };

    let Some(first) = Date::from_calendar_date(year, month_value, 1).ok() else {
        return String::new();
    };

    let offset = first.weekday().number_from_monday() - 1;
    let entry_days: BTreeSet<&str> = entries.iter().map(|entry| entry.iso.as_str()).collect();
    let mut html = String::new();

    let _ = write!(
        html,
        r#"<div class="flex items-center justify-between px-1 pb-1">
  <span class="font-ui text-[10px] font-semibold text-on-surface">{} {}</span>
  <div class="flex items-center gap-1 text-on-surface-variant">
    <button type="button" data-calendar-nav="-1" aria-label="Previous month" class="px-1 text-xs leading-none hover:text-on-surface">‹</button>
    <button type="button" data-calendar-nav="1" aria-label="Next month" class="px-1 text-xs leading-none hover:text-on-surface">›</button>
  </div>
</div>
<div class="grid grid-cols-7 gap-1 text-center font-ui text-[9px] text-on-surface-variant">
  <span>M</span><span>T</span><span>W</span><span>T</span><span>F</span><span>S</span><span>S</span>
</div>
<div class="grid grid-cols-7 gap-1 pt-1 text-center font-ui text-[10px]">"#,
        month_name(month_value),
        year
    );

    for _ in 0..offset {
        html.push_str("<span></span>");
    }

    for day in 1..=31u32 {
        let Ok(date) = Date::from_calendar_date(year, month_value, day as u8) else {
            break;
        };

        let iso = date.to_string();
        let is_future = iso.as_str() > today_iso;
        let is_selected = iso == selected_iso;
        let has_entry = entry_days.contains(iso.as_str());

        let mut class = String::from(
            "relative flex h-6 w-6 items-center justify-center rounded transition-colors",
        );

        if is_future {
            class.push_str(" cursor-default text-on-surface-variant/35");
        } else if is_selected {
            class.push_str(" bg-primary text-on-primary hover:bg-primary");
        } else {
            class.push_str(" text-on-surface-variant hover:bg-surface-container");

            if iso == today_iso {
                class.push_str(" font-semibold text-primary");
            }
        }

        if has_entry && !is_selected {
            class.push_str(
                " after:absolute after:bottom-0.5 after:left-1/2 after:h-1 after:w-1 after:-translate-x-1/2 after:rounded-full after:bg-primary",
            );
        }

        let disabled = if is_future { " disabled" } else { "" };
        let pressed = if is_selected { "true" } else { "false" };

        let _ = write!(
            html,
            r#"<button type="button" data-date="{iso}" aria-label="{iso}" aria-pressed="{pressed}" class="{class}"{disabled}>{day}</button>"#
        );
    }

    html.push_str("</div>");

    html
}

fn month_name(month: Month) -> &'static str {
    match month {
        Month::January => "January",
        Month::February => "February",
        Month::March => "March",
        Month::April => "April",
        Month::May => "May",
        Month::June => "June",
        Month::July => "July",
        Month::August => "August",
        Month::September => "September",
        Month::October => "October",
        Month::November => "November",
        Month::December => "December",
    }
}

fn escape_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
