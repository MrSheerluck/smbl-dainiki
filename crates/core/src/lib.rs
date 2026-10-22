use time::{Date, Month, Weekday, macros::format_description};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryMeta {
    pub date: Date,
    pub excerpt: String,
    pub tags: Vec<String>,
    pub words: usize,
}

pub fn parse_date(input: &str) -> Option<Date> {
    Date::parse(input, format_description!("[year]-[month]-[day]")).ok()
}

pub fn entry_path(date: Date) -> String {
    let year = date.year();
    let month = date.month() as u8;
    let day = date.day();

    format!("{year:04}/{month:02}/{year:04}-{month:02}-{day:02}.md")
}

pub fn parse_entry_path(path: &str) -> Option<Date> {
    let file = path.rsplit('/').next()?;
    let stem = file.strip_suffix(".md")?;
    let date = parse_date(stem)?;

    if entry_path(date) == path {
        Some(date)
    } else {
        None
    }
}

pub fn weekday_name(date: Date) -> &'static str {
    match date.weekday() {
        Weekday::Monday => "Monday",
        Weekday::Tuesday => "Tuesday",
        Weekday::Wednesday => "Wednesday",
        Weekday::Thursday => "Thursday",
        Weekday::Friday => "Friday",
        Weekday::Saturday => "Saturday",
        Weekday::Sunday => "Sunday",
    }
}

pub fn month_short(date: Date) -> &'static str {
    match date.month() {
        Month::January => "Jan",
        Month::February => "Feb",
        Month::March => "Mar",
        Month::April => "Apr",
        Month::May => "May",
        Month::June => "Jun",
        Month::July => "Jul",
        Month::August => "Aug",
        Month::September => "Sep",
        Month::October => "Oct",
        Month::November => "Nov",
        Month::December => "Dec",
    }
}

pub fn month_year(date: Date) -> String {
    format!("{} {}", month_short(date), date.year())
}

pub fn parse_entry(text: &str, date: Date) -> EntryMeta {
    let excerpt = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty() && !line.starts_with('#'))
        .unwrap_or("")
        .to_string();

    EntryMeta {
        date,
        excerpt,
        tags: extract_tags(text),
        words: text.split_whitespace().count(),
    }
}

pub fn extract_tags(text: &str) -> Vec<String> {
    let mut tags: Vec<String> = Vec::new();
    let mut boundary = true;
    let mut chars = text.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '#' && boundary {
            let mut tag = String::new();

            while let Some(&next) = chars.peek() {
                if next.is_alphanumeric() || next == '-' || next == '_' {
                    tag.push(next);
                    chars.next();
                } else {
                    break;
                }
            }

            if !tag.is_empty() {
                let normalized = tag.to_lowercase();
                if !tags.contains(&normalized) {
                    tags.push(normalized);
                }
            }

            boundary = false;
        } else {
            boundary = is_boundary(ch);
        }
    }

    tags
}

fn is_boundary(ch: char) -> bool {
    ch.is_whitespace() || matches!(ch, '(' | '[' | '{' | '>' | ',' | ';')
}

#[cfg(test)]
mod tests {
    use super::*;
    use time::macros::date;

    #[test]
    fn builds_canonical_entry_path() {
        let date = date!(2026 - 10 - 22);
        assert_eq!(entry_path(date), "2026/10/2026-10-22.md");
    }

    #[test]
    fn parses_canonical_entry_path() {
        let date = parse_entry_path("2026/10/2026-10-22.md").unwrap();
        assert_eq!(date, date!(2026 - 10 - 22));
    }

    #[test]
    fn rejects_mismatched_paths() {
        assert!(parse_entry_path("2026/11/2026-10-22.md").is_none());
        assert!(parse_entry_path("2026-10-22.md").is_none());
    }

    #[test]
    fn formats_display_parts() {
        let date = date!(2026 - 10 - 22);
        assert_eq!(weekday_name(date), "Thursday");
        assert_eq!(month_year(date), "Oct 2026");
    }

    #[test]
    fn extracts_unique_tags_in_order() {
        let tags = extract_tags("Morning. #Morning #reflections, then #morning again.");
        assert_eq!(tags, vec!["morning", "reflections"]);
    }

    #[test]
    fn ignores_headings_and_url_fragments() {
        assert!(extract_tags("# Heading\n\nhttps://example.com/#anchor").is_empty());
    }

    #[test]
    fn supports_punctuation_boundaries_and_hyphens() {
        let tags = extract_tags("(#morning) #deep-work #日本語");
        assert_eq!(tags, vec!["morning", "deep-work", "日本語"]);
    }

    #[test]
    fn parses_excerpt_tags_and_words() {
        let text = "The morning came in quietly.\n\nA second paragraph.\n\n#morning #reflections\n";
        let entry = parse_entry(text, date!(2026 - 10 - 22));

        assert_eq!(entry.excerpt, "The morning came in quietly.");
        assert_eq!(entry.tags.join(","), "morning,reflections");
        assert!(entry.words > 0);
    }

    #[test]
    fn skips_tag_only_lines_for_excerpt() {
        let text = "#morning\nThe real first line.\n";
        let entry = parse_entry(text, date!(2026 - 10 - 22));

        assert_eq!(entry.excerpt, "The real first line.");
    }
}
