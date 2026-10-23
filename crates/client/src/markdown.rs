use wasm_bindgen::JsCast;
use web_sys::Element;

pub fn render_document(editor: &Element, markdown: &str) {
    editor.set_inner_html(&render_document_html(markdown));
}

pub fn render_document_html(markdown: &str) -> String {
    let mut html = String::new();

    for line in markdown.split('\n') {
        let (class, inner) = render_line(line);
        html.push_str(&format!(r#"<div class="line {class}">{inner}</div>"#));
    }

    html
}

pub fn render_line_into(line: &Element, source: &str, active: bool) {
    let (class, inner) = render_line(source);
    let mut classes = String::from("line");

    if !class.is_empty() {
        classes.push(' ');
        classes.push_str(&class);
    }

    if active {
        classes.push_str(" active");
    }

    line.set_class_name(&classes);
    line.set_inner_html(&inner);
}

pub fn source_from_editor(editor: &Element) -> String {
    let mut lines = Vec::new();
    let mut child = editor.first_element_child();

    while let Some(line) = child {
        if line.class_list().contains("line") {
            lines.push(line.text_content().unwrap_or_default());
        }

        child = line.next_element_sibling();
    }

    lines.join("\n")
}

pub fn line_of(node: &web_sys::Node) -> Option<Element> {
    let element = node
        .dyn_ref::<Element>()
        .cloned()
        .or_else(|| node.parent_element())
        .or_else(|| node.parent_node()?.dyn_into::<Element>().ok())?;

    element.closest(".line").ok().flatten()
}

fn render_line(source: &str) -> (String, String) {
    if source.starts_with("```") {
        return ("md-code".to_string(), escape(source));
    }

    if let Some((level, rest)) = heading(source) {
        let mut html = marker(&source[..level + 1]);
        inline_into(rest, &mut html);
        return (format!("md-h{level}"), html);
    }

    if let Some(rest) = source.strip_prefix("> ") {
        let mut html = marker("> ");
        inline_into(rest, &mut html);
        return ("md-quote".to_string(), html);
    }

    if let Some((prefix, rest)) = list_prefix(source) {
        let mut html = marker(prefix);
        inline_into(rest, &mut html);
        return ("md-list".to_string(), html);
    }

    if is_rule(source) {
        return ("md-hr".to_string(), marker(source));
    }

    let mut html = String::new();
    inline_into(source, &mut html);

    if html.is_empty() {
        html.push_str("<br>");
    }

    (String::new(), html)
}

fn heading(source: &str) -> Option<(usize, &str)> {
    let hashes = source.chars().take_while(|ch| *ch == '#').count();

    if (1..=6).contains(&hashes) && source[hashes..].starts_with(' ') {
        Some((hashes, &source[hashes + 1..]))
    } else {
        None
    }
}

fn list_prefix(source: &str) -> Option<(&str, &str)> {
    for prefix in ["- ", "* ", "+ "] {
        if let Some(rest) = source.strip_prefix(prefix) {
            return Some((prefix, rest));
        }
    }

    let digits = source.chars().take_while(|ch| ch.is_ascii_digit()).count();

    if digits > 0 && source[digits..].starts_with(". ") {
        let length = digits + 2;
        return Some((&source[..length], &source[length..]));
    }

    None
}

fn is_rule(source: &str) -> bool {
    let trimmed = source.trim();
    let Some(first) = trimmed.chars().next() else {
        return false;
    };

    matches!(first, '-' | '*' | '_')
        && trimmed.chars().count() >= 3
        && trimmed.chars().all(|ch| ch == first || ch == ' ')
}

fn inline_into(text: &str, out: &mut String) {
    let mut pos = 0;

    while pos < text.len() {
        let rest = &text[pos..];

        if let Some((consumed, html)) = parse_link(rest) {
            out.push_str(&html);
            pos += consumed;
            continue;
        }

        if let Some((consumed, html)) = parse_pair(rest, "`", "md-code-inline", false) {
            out.push_str(&html);
            pos += consumed;
            continue;
        }

        if let Some((consumed, html)) = parse_pair(rest, "**", "md-strong", true) {
            out.push_str(&html);
            pos += consumed;
            continue;
        }

        if let Some((consumed, html)) = parse_pair(rest, "~~", "md-strike", true) {
            out.push_str(&html);
            pos += consumed;
            continue;
        }

        if !rest.starts_with("**") {
            if let Some((consumed, html)) = parse_pair(rest, "*", "md-em", true) {
                out.push_str(&html);
                pos += consumed;
                continue;
            }
        }

        if !rest.starts_with("__") {
            if let Some((consumed, html)) = parse_pair(rest, "_", "md-em", true) {
                out.push_str(&html);
                pos += consumed;
                continue;
            }
        }

        let ch = rest.chars().next().unwrap();

        if ch == '#' && is_boundary(text, pos) {
            if let Some((consumed, html)) = parse_tag(rest) {
                out.push_str(&html);
                pos += consumed;
                continue;
            }
        }

        out.push_str(&escape_char(ch));
        pos += ch.len_utf8();
    }
}

fn parse_link(input: &str) -> Option<(usize, String)> {
    if !input.starts_with('[') {
        return None;
    }

    let label_end = input.find("](")?;
    let label = &input[1..label_end];
    let after = &input[label_end + 2..];
    let url_end = after.find(')')?;
    let url = &after[..url_end];
    let consumed = label_end + 2 + url_end + 1;

    let mut html = marker("[");
    html.push_str(r#"<span class="md-link">"#);
    inline_into(label, &mut html);
    html.push_str("</span>");
    html.push_str(&marker(&format!("]({url})")));

    Some((consumed, html))
}

fn parse_pair(
    input: &str,
    delimiter: &str,
    class: &str,
    recurse: bool,
) -> Option<(usize, String)> {
    if !input.starts_with(delimiter) {
        return None;
    }

    let after = &input[delimiter.len()..];
    let end = after.find(delimiter)?;

    if end == 0 {
        return None;
    }

    let inner = &after[..end];
    let consumed = delimiter.len() + end + delimiter.len();

    let mut html = marker(delimiter);
    html.push_str(&format!(r#"<span class="{class}">"#));

    if recurse {
        inline_into(inner, &mut html);
    } else {
        html.push_str(&escape(inner));
    }

    html.push_str("</span>");
    html.push_str(&marker(delimiter));

    Some((consumed, html))
}

fn parse_tag(input: &str) -> Option<(usize, String)> {
    let mut length = 1;

    for ch in input[1..].chars() {
        if ch.is_alphanumeric() || ch == '-' || ch == '_' {
            length += ch.len_utf8();
        } else {
            break;
        }
    }

    if length == 1 {
        return None;
    }

    let tag = &input[..length];
    Some((length, format!(r#"<span class="md-tag">{}</span>"#, escape(tag))))
}

fn is_boundary(text: &str, pos: usize) -> bool {
    if pos == 0 {
        return true;
    }

    text[..pos]
        .chars()
        .next_back()
        .is_none_or(|ch| ch.is_whitespace() || matches!(ch, '(' | '[' | '{' | '>' | ',' | ';'))
}

fn marker(text: &str) -> String {
    format!(r#"<span class="marker">{}</span>"#, escape(text))
}

fn escape(text: &str) -> String {
    let mut output = String::with_capacity(text.len());

    for ch in text.chars() {
        output.push_str(&escape_char(ch));
    }

    output
}

fn escape_char(ch: char) -> String {
    match ch {
        '&' => "&amp;".to_string(),
        '<' => "&lt;".to_string(),
        '>' => "&gt;".to_string(),
        '"' => "&quot;".to_string(),
        _ => ch.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text_content(html: &str) -> String {
        let mut output = String::new();
        let mut in_tag = false;

        for ch in html.chars() {
            match ch {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => output.push(ch),
                _ => {}
            }
        }

        output
            .replace("&amp;", "&")
            .replace("&lt;", "<")
            .replace("&gt;", ">")
            .replace("&quot;", "\"")
    }

    #[test]
    fn rendered_html_preserves_source_text() {
        for source in [
            "# Heading",
            "A **bold** and *italic* and ~~strike~~ and `code`.",
            "> quote",
            "- bullet",
            "1. ordered",
            "[link](https://example.com) #tag",
            "---",
            "",
        ] {
            let (_, html) = render_line(source);
            assert_eq!(text_content(&html), source, "source: {source:?}");
        }
    }
}
