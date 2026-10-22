use dainiki_core::{parse_date, parse_entry};
use serde::Serialize;
use wasm_bindgen::prelude::*;

#[derive(Serialize)]
struct EntryJson {
    date: String,
    excerpt: String,
    tags: Vec<String>,
    words: usize,
}

#[wasm_bindgen]
pub fn parse_entry_json(text: &str, date: &str) -> Result<String, JsError> {
    let date = parse_date(date).ok_or_else(|| JsError::new("invalid date"))?;
    let entry = parse_entry(text, date);

    let payload = EntryJson {
        date: date.to_string(),
        excerpt: entry.excerpt,
        tags: entry.tags,
        words: entry.words,
    };

    serde_json::to_string(&payload).map_err(|error| JsError::new(&error.to_string()))
}
