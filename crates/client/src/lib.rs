mod markdown;
mod opfs;
mod render;

use std::{
    cell::RefCell,
    future::Future,
    io::{Cursor, Read, Write},
    rc::Rc,
};

use dainiki_core::{month_year, parse_date, parse_entry, weekday_name};
use gloo_timers::callback::Timeout;
use js_sys::Uint8Array;
use render::EntryRecord;
use time::Duration;
use wasm_bindgen::{JsCast, JsValue, prelude::*};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{
    Blob, ClipboardEvent, Document, Element, Event, File, HtmlAnchorElement, HtmlElement,
    HtmlInputElement, HtmlTextAreaElement, KeyboardEvent, Node, Url,
};
use zip::{CompressionMethod, ZipArchive, ZipWriter, write::SimpleFileOptions};

struct State {
    selected_iso: String,
    today_iso: String,
    calendar_year: i32,
    calendar_month: u32,
    entries: Vec<EntryRecord>,
    timeline_limit: usize,
    active_tag: Option<String>,
    has_file: bool,
    base_text: String,
    saving: bool,
    composing: bool,
    undo_stack: Vec<String>,
    redo_stack: Vec<String>,
    last_snapshot: String,
    editor: HtmlElement,
    status: Element,
    editor_tags: Element,
    timeline: Element,
    tag_list: Element,
    calendar: Element,
    active_filter: Element,
    active_filter_label: Element,
    search_overlay: Element,
    search_input: HtmlInputElement,
    search_results: Element,
    search_matches: Vec<String>,
    search_selected: usize,
    data_overlay: Element,
    data_status: Element,
    import_file: HtmlInputElement,
    import_folder: HtmlInputElement,
    inline_iso: Option<String>,
    inline_base: String,
    inline_link: Option<Element>,
    inline_container: Option<Element>,
    inline_textarea: Option<HtmlTextAreaElement>,
    inline_status: Option<Element>,
    inline_saving: bool,
    inline_save_timeout: Option<Timeout>,
    sidebar: Element,
    sidebar_backdrop: Element,
    day_label: Element,
    weekday_label: Element,
    month_label: Element,
    today_button: Element,
    next_day_button: Element,
    save_timeout: Option<Timeout>,
    history_timeout: Option<Timeout>,
}

type SharedState = Rc<RefCell<State>>;

fn spawn_result<F>(future: F)
where
    F: Future<Output = Result<(), JsValue>> + 'static,
{
    spawn_local(async move {
        if let Err(error) = future.await {
            web_sys::console::error_1(&error);
        }
    });
}

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let document = window
        .document()
        .ok_or_else(|| JsValue::from_str("no document"))?;

    let editor = query::<HtmlElement>(&document, "#entry-editor")?;
    let today_iso = today_iso();
    let (calendar_year, calendar_month) = month_parts(&today_iso);

    let state = Rc::new(RefCell::new(State {
        selected_iso: today_iso.clone(),
        today_iso,
        calendar_year,
        calendar_month,
        entries: Vec::new(),
        timeline_limit: 30,
        active_tag: None,
        has_file: false,
        base_text: String::new(),
        saving: false,
        composing: false,
        undo_stack: Vec::new(),
        redo_stack: Vec::new(),
        last_snapshot: String::new(),
        editor,
        status: element(&document, "#entry-status")?,
        editor_tags: element(&document, "#entry-tags")?,
        timeline: element(&document, "#timeline-entries")?,
        tag_list: element(&document, "#tag-list")?,
        calendar: element(&document, "#calendar")?,
        active_filter: element(&document, "#active-filter")?,
        active_filter_label: element(&document, "#active-filter-label")?,
        search_overlay: element(&document, "#search-overlay")?,
        search_input: query::<HtmlInputElement>(&document, "#search-input")?,
        search_results: element(&document, "#search-results")?,
        search_matches: Vec::new(),
        search_selected: 0,
        data_overlay: element(&document, "#data-overlay")?,
        data_status: element(&document, "#data-status")?,
        import_file: query::<HtmlInputElement>(&document, "#import-file")?,
        import_folder: query::<HtmlInputElement>(&document, "#import-folder-input")?,
        inline_iso: None,
        inline_base: String::new(),
        inline_link: None,
        inline_container: None,
        inline_textarea: None,
        inline_status: None,
        inline_saving: false,
        inline_save_timeout: None,
        sidebar: element(&document, "aside")?,
        sidebar_backdrop: element(&document, "#sidebar-backdrop")?,
        day_label: element(&document, "#today-day")?,
        weekday_label: element(&document, "#today-weekday")?,
        month_label: element(&document, "#today-month")?,
        today_button: element(&document, "#today-button")?,
        next_day_button: element(&document, "#next-day")?,
        save_timeout: None,
        history_timeout: None,
    }));

    init_theme(&document)?;
    bind_events(&state, &document)?;
    update_labels(&state);

    spawn_result(register_service_worker());
    spawn_result(init(state.clone()));

    Ok(())
}

async fn register_service_worker() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let container = window.navigator().service_worker();

    JsFuture::from(container.register("/sw.js")).await?;
    Ok(())
}

fn init_theme(document: &Document) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let saved = window
        .local_storage()?
        .and_then(|storage| storage.get_item("dainiki-theme").ok().flatten());

    let system_dark = window
        .match_media("(prefers-color-scheme: dark)")?
        .is_some_and(|query| query.matches());

    let theme = match saved.as_deref() {
        Some("dark") => "dark",
        Some("light") => "light",
        _ if system_dark => "dark",
        _ => "light",
    };

    apply_theme(document, theme)
}

fn apply_theme(document: &Document, theme: &str) -> Result<(), JsValue> {
    let root = document
        .document_element()
        .ok_or_else(|| JsValue::from_str("no document element"))?;
    root.set_attribute("data-theme", theme)?;

    if let Some(button) = document.query_selector("#theme-toggle")? {
        let (icon, label) = if theme == "dark" {
            ("☀", "Switch to light theme")
        } else {
            ("☾", "Switch to dark theme")
        };

        button.set_text_content(Some(icon));
        button.set_attribute("aria-label", label)?;
    }

    Ok(())
}

fn toggle_theme(document: &Document) -> Result<(), JsValue> {
    let root = document
        .document_element()
        .ok_or_else(|| JsValue::from_str("no document element"))?;
    let current = root
        .get_attribute("data-theme")
        .unwrap_or_else(|| "light".to_string());
    let next = if current == "dark" { "light" } else { "dark" };

    apply_theme(document, next)?;

    if let Some(storage) = web_sys::window()
        .ok_or_else(|| JsValue::from_str("no window"))?
        .local_storage()?
    {
        storage.set_item("dainiki-theme", next)?;
    }

    Ok(())
}

async fn init(state: SharedState) -> Result<(), JsValue> {
    let iso = state.borrow().selected_iso.clone();
    load_entry(state.clone(), &iso).await?;
    refresh(state.clone()).await
}

fn bind_events(state: &SharedState, document: &Document) -> Result<(), JsValue> {
    let editor = state.borrow().editor.clone();

    let input_state = state.clone();
    listen(
        editor.as_ref(),
        "input",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            if input_state.borrow().composing {
                return;
            }

            schedule_save(input_state.clone());
            schedule_history(input_state.clone());
        }),
    )?;

    let composition_start = state.clone();
    listen(
        editor.as_ref(),
        "compositionstart",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            composition_start.borrow_mut().composing = true;
        }),
    )?;

    let composition_end = state.clone();
    listen(
        editor.as_ref(),
        "compositionend",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            composition_end.borrow_mut().composing = false;
            schedule_save(composition_end.clone());
            schedule_history(composition_end.clone());
        }),
    )?;

    let paste_state = state.clone();
    let paste_document = document.clone();
    listen(
        editor.as_ref(),
        "paste",
        Closure::<dyn FnMut(Event)>::new(move |event| {
            handle_paste(paste_state.clone(), &paste_document, event);
        }),
    )?;

    let blur_state = state.clone();
    listen(
        editor.as_ref(),
        "blur",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            spawn_result(save_entry(blur_state.clone()));
        }),
    )?;

    let focus_state = state.clone();
    listen(
        editor.as_ref(),
        "mousedown",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            if focus_state.borrow().inline_iso.is_some() {
                spawn_result(finish_inline_edit(focus_state.clone(), true));
            }
        }),
    )?;

    let key_state = state.clone();
    let key_document = document.clone();
    listen(
        editor.as_ref(),
        "keydown",
        Closure::<dyn FnMut(Event)>::new(move |event| {
            handle_keydown(key_state.clone(), &key_document, event);
        }),
    )?;

    let selection_state = state.clone();
    let selection_document = document.clone();
    listen(
        document.as_ref(),
        "selectionchange",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            if selection_state.borrow().composing {
                return;
            }

            update_active_line(selection_state.clone(), &selection_document);
        }),
    )?;

    let tag_state = state.clone();
    listen(
        state.borrow().tag_list.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |event| {
            let Some(target) = target_element(&event) else {
                return;
            };

            let Ok(Some(tag)) = target.closest("[data-tag]") else {
                return;
            };

            let Some(value) = tag.get_attribute("data-tag") else {
                return;
            };

            let current = tag_state.borrow().active_tag.clone();
            let next = if current.as_deref() == Some(value.as_str()) {
                None
            } else {
                Some(value)
            };

            set_active_tag(tag_state.clone(), next);
        }),
    )?;

    let timeline_state = state.clone();
    listen(
        state.borrow().timeline.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |event| {
            let Some(target) = target_element(&event) else {
                return;
            };

            if target.closest("[data-load-older]").ok().flatten().is_some() {
                {
                    let mut state = timeline_state.borrow_mut();
                    state.timeline_limit += 30;
                }
                render_ui(timeline_state.clone());
                return;
            }

            let Ok(Some(link)) = target.closest("[data-iso]") else {
                return;
            };

            let Some(iso) = link.get_attribute("data-iso") else {
                return;
            };

            event.prevent_default();
            spawn_result(start_inline_edit(timeline_state.clone(), iso));
        }),
    )?;

    let calendar_state = state.clone();
    listen(
        state.borrow().calendar.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |event| {
            let Some(target) = target_element(&event) else {
                return;
            };

            if let Ok(Some(nav)) = target.closest("[data-calendar-nav]") {
                if let Some(value) = nav.get_attribute("data-calendar-nav") {
                    if let Ok(delta) = value.parse::<i32>() {
                        shift_month(calendar_state.clone(), delta);
                    }
                }
                return;
            }

            let Ok(Some(day)) = target.closest("[data-date]") else {
                return;
            };

            if let Some(iso) = day.get_attribute("data-date") {
                let exists = calendar_state
                    .borrow()
                    .entries
                    .iter()
                    .any(|entry| entry.iso == iso);

                if exists {
                    open_entry(calendar_state.clone(), iso);
                } else {
                    spawn_result(select_date(calendar_state.clone(), iso));
                }
            }
        }),
    )?;

    let today_state = state.clone();
    listen(
        state.borrow().today_button.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            let today = today_state.borrow().today_iso.clone();
            spawn_result(select_date(today_state.clone(), today));
        }),
    )?;

    let previous_state = state.clone();
    listen(
        element(document, "#prev-day")?.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            shift_date(previous_state.clone(), -1);
        }),
    )?;

    let next_state = state.clone();
    listen(
        element(document, "#next-day")?.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            shift_date(next_state.clone(), 1);
        }),
    )?;

    let clear_state = state.clone();
    listen(
        element(document, "#clear-filter")?.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            set_active_tag(clear_state.clone(), None);
        }),
    )?;

    let theme_document = document.clone();
    listen(
        element(document, "#theme-toggle")?.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            if let Err(error) = toggle_theme(&theme_document) {
                web_sys::console::error_1(&error);
            }
        }),
    )?;

    let open_state = state.clone();
    listen(
        element(document, "#open-search")?.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            open_search(open_state.clone());
        }),
    )?;

    let global_state = state.clone();
    listen(
        document.as_ref(),
        "keydown",
        Closure::<dyn FnMut(Event)>::new(move |event| {
            handle_global_keydown(global_state.clone(), event);
        }),
    )?;

    let overlay_state = state.clone();
    listen(
        state.borrow().search_overlay.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |event| {
            let Some(target) = target_element(&event) else {
                return;
            };

            if target.id() == "search-overlay" {
                close_search(overlay_state.clone());
            }
        }),
    )?;

    let search_input_state = state.clone();
    listen(
        state.borrow().search_input.as_ref(),
        "input",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            search_input_state.borrow_mut().search_selected = 0;
            render_search_results(search_input_state.clone());
        }),
    )?;

    let search_key_state = state.clone();
    listen(
        state.borrow().search_input.as_ref(),
        "keydown",
        Closure::<dyn FnMut(Event)>::new(move |event| {
            handle_search_keydown(search_key_state.clone(), event);
        }),
    )?;

    let search_result_state = state.clone();
    listen(
        state.borrow().search_results.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |event| {
            let Some(target) = target_element(&event) else {
                return;
            };

            let Ok(Some(result)) = target.closest("[data-iso]") else {
                return;
            };

            let Some(iso) = result.get_attribute("data-iso") else {
                return;
            };

            close_search(search_result_state.clone());
            open_entry(search_result_state.clone(), iso);
        }),
    )?;

    let open_sidebar_state = state.clone();
    listen(
        element(document, "#open-sidebar")?.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            open_sidebar(open_sidebar_state.clone());
        }),
    )?;

    let close_sidebar_state = state.clone();
    listen(
        element(document, "#close-sidebar")?.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            close_sidebar(close_sidebar_state.clone());
        }),
    )?;

    let sidebar_backdrop_state = state.clone();
    listen(
        state.borrow().sidebar_backdrop.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            close_sidebar(sidebar_backdrop_state.clone());
        }),
    )?;

    let open_data_state = state.clone();
    listen(
        element(document, "#open-data")?.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            open_data(open_data_state.clone());
        }),
    )?;

    let close_data_state = state.clone();
    listen(
        element(document, "#close-data")?.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            close_data(close_data_state.clone());
        }),
    )?;

    let data_overlay_state = state.clone();
    listen(
        state.borrow().data_overlay.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |event| {
            let Some(target) = target_element(&event) else {
                return;
            };

            if target.id() == "data-overlay" {
                close_data(data_overlay_state.clone());
            }
        }),
    )?;

    let export_state = state.clone();
    listen(
        element(document, "#export-zip")?.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            spawn_result(export_zip(export_state.clone()));
        }),
    )?;

    let import_zip_state = state.clone();
    listen(
        element(document, "#import-zip")?.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            let input = import_zip_state.borrow().import_file.clone();
            input.click();
        }),
    )?;

    let import_folder_state = state.clone();
    listen(
        element(document, "#import-folder")?.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            let input = import_folder_state.borrow().import_folder.clone();
            input.click();
        }),
    )?;

    let import_file_state = state.clone();
    listen(
        state.borrow().import_file.as_ref(),
        "change",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            let input = import_file_state.borrow().import_file.clone();
            spawn_result(import_files(import_file_state.clone(), input));
        }),
    )?;

    let import_folder_input_state = state.clone();
    listen(
        state.borrow().import_folder.as_ref(),
        "change",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            let input = import_folder_input_state.borrow().import_folder.clone();
            spawn_result(import_files(import_folder_input_state.clone(), input));
        }),
    )?;

    let visibility_state = state.clone();
    let visibility_document = document.clone();
    listen(
        document.as_ref(),
        "visibilitychange",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            if visibility_document.hidden() {
                spawn_result(save_entry(visibility_state.clone()));
            }
        }),
    )?;

    Ok(())
}

fn listen(
    target: &web_sys::EventTarget,
    event: &str,
    callback: Closure<dyn FnMut(Event)>,
) -> Result<(), JsValue> {
    target.add_event_listener_with_callback(event, callback.as_ref().unchecked_ref())?;
    callback.forget();
    Ok(())
}

fn handle_keydown(state: SharedState, document: &Document, event: Event) {
    let Ok(event) = event.dyn_into::<KeyboardEvent>() else {
        return;
    };

    if event.is_composing() {
        return;
    }

    let modifier = event.meta_key() || event.ctrl_key();

    if modifier && event.key().eq_ignore_ascii_case("z") {
        event.prevent_default();

        if event.shift_key() {
            redo(state, document);
        } else {
            undo(state, document);
        }

        return;
    }

    if modifier && event.key().eq_ignore_ascii_case("y") {
        event.prevent_default();
        redo(state, document);
        return;
    }

    if event.key() == "Enter" {
        event.prevent_default();
        split_line(state, document);
        return;
    }

    if event.key() == "Backspace" && merge_line(state, document) {
        event.prevent_default();
    }
}

fn handle_global_keydown(state: SharedState, event: Event) {
    let Ok(event) = event.dyn_into::<KeyboardEvent>() else {
        return;
    };

    if event.is_composing() {
        return;
    }

    let modifier = event.meta_key() || event.ctrl_key();

    if modifier && event.key().eq_ignore_ascii_case("k") {
        event.prevent_default();
        open_search(state);
    }
}

fn open_search(state: SharedState) {
    let overlay = state.borrow().search_overlay.clone();
    overlay.remove_attribute("hidden").ok();

    let input = state.borrow().search_input.clone();
    input.focus().ok();
    input.select();
    state.borrow_mut().search_selected = 0;
    render_search_results(state);
}

fn close_search(state: SharedState) {
    let overlay = state.borrow().search_overlay.clone();
    overlay.set_attribute("hidden", "true").ok();

    let editor = state.borrow().editor.clone();
    editor.focus().ok();
}

fn render_search_results(state: SharedState) {
    let (html, matches, selected) = {
        let state = state.borrow();
        render::search_results(
            &state.entries,
            &state.search_input.value(),
            state.search_selected,
        )
    };

    let mut state = state.borrow_mut();
    state.search_results.set_inner_html(&html);
    state.search_matches = matches;
    state.search_selected = selected;
}

fn handle_search_keydown(state: SharedState, event: Event) {
    let Ok(event) = event.dyn_into::<KeyboardEvent>() else {
        return;
    };

    if event.is_composing() {
        return;
    }

    match event.key().as_str() {
        "Escape" => {
            event.prevent_default();
            close_search(state);
        }
        "ArrowDown" => {
            event.prevent_default();
            let length = state.borrow().search_matches.len();

            if length > 0 {
                let mut borrowed = state.borrow_mut();
                borrowed.search_selected = (borrowed.search_selected + 1).min(length - 1);
                drop(borrowed);
                render_search_results(state.clone());
            }
        }
        "ArrowUp" => {
            event.prevent_default();
            let mut borrowed = state.borrow_mut();
            borrowed.search_selected = borrowed.search_selected.saturating_sub(1);
            drop(borrowed);
            render_search_results(state.clone());
        }
        "Enter" => {
            event.prevent_default();
            let iso = {
                let state = state.borrow();
                state.search_matches.get(state.search_selected).cloned()
            };

            if let Some(iso) = iso {
                close_search(state.clone());
                open_entry(state, iso);
            }
        }
        _ => {}
    }
}

#[derive(Clone, Copy)]
enum ImportOutcome {
    Imported,
    Conflict,
    Unchanged,
}

fn open_sidebar(state: SharedState) {
    let state = state.borrow();
    state.sidebar.class_list().remove_1("hidden").ok();
    state.sidebar.class_list().add_1("flex").ok();
    state
        .sidebar_backdrop
        .remove_attribute("hidden")
        .ok();
}

fn close_sidebar(state: SharedState) {
    let state = state.borrow();
    state.sidebar.class_list().add_1("hidden").ok();
    state.sidebar.class_list().remove_1("flex").ok();
    state
        .sidebar_backdrop
        .set_attribute("hidden", "true")
        .ok();
}

fn open_data(state: SharedState) {
    state.borrow().data_overlay.remove_attribute("hidden").ok();
    set_data_status(&state, "Markdown only. Media files are ignored.");
}

fn close_data(state: SharedState) {
    state
        .borrow()
        .data_overlay
        .set_attribute("hidden", "true")
        .ok();
}

fn set_data_status(state: &SharedState, text: &str) {
    state.borrow().data_status.set_text_content(Some(text));
}

async fn export_zip(state: SharedState) -> Result<(), JsValue> {
    set_data_status(&state, "Preparing export…");

    let entries = opfs::list_entries().await?;
    let count = entries.len();
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));

    for (iso, text) in entries {
        let path = format!("{}/{}/{}.md", &iso[0..4], &iso[5..7], iso);
        writer.start_file(path, options).map_err(zip_error)?;
        writer.write_all(text.as_bytes()).map_err(io_error)?;
    }

    writer
        .start_file("README.md", options)
        .map_err(zip_error)?;
    writer
        .write_all(
            b"# Dainiki journal\n\nEntries are Markdown files in `YYYY/MM/YYYY-MM-DD.md`.\nDainiki is text-only; media files are not included.\n",
        )
        .map_err(io_error)?;

    let cursor = writer.finish().map_err(zip_error)?;
    let bytes = cursor.into_inner();
    let document = state
        .borrow()
        .editor
        .owner_document()
        .ok_or_else(|| JsValue::from_str("no document"))?;

    download_bytes(&document, &bytes, "dainiki-journal.zip")?;
    set_data_status(&state, &format!("Exported {count} entries."));
    Ok(())
}

async fn import_files(state: SharedState, input: HtmlInputElement) -> Result<(), JsValue> {
    let Some(files) = input.files() else {
        set_data_status(&state, "No files selected.");
        return Ok(());
    };

    let mut imported = 0;
    let mut conflicts = 0;
    let mut skipped = 0;

    for index in 0..files.length() {
        let Some(file) = files.item(index) else {
            continue;
        };

        let name = file.name();
        let lower = name.to_lowercase();

        if lower.ends_with(".zip") {
            let bytes = read_file_bytes(&file).await?;
            let (zip_imported, zip_conflicts, zip_skipped) = import_zip(bytes).await?;
            imported += zip_imported;
            conflicts += zip_conflicts;
            skipped += zip_skipped;
        } else if lower.ends_with(".md") {
            let Some(iso) = date_from_stem(&name) else {
                skipped += 1;
                continue;
            };

            let text = read_file_text(&file).await?;

            match import_markdown(&iso, &text).await? {
                ImportOutcome::Imported => imported += 1,
                ImportOutcome::Conflict => conflicts += 1,
                ImportOutcome::Unchanged => {}
            }
        } else {
            skipped += 1;
        }
    }

    input.set_value("");
    let selected = state.borrow().selected_iso.clone();
    load_entry(state.clone(), &selected).await?;
    refresh(state.clone()).await?;
    set_data_status(
        &state,
        &format!("Imported {imported}, conflicts {conflicts}, skipped {skipped}."),
    );
    Ok(())
}

async fn import_zip(bytes: Vec<u8>) -> Result<(usize, usize, usize), JsValue> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(zip_error)?;
    let mut imported = 0;
    let mut conflicts = 0;
    let mut skipped = 0;

    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(zip_error)?;
        let name = file.name().map_err(zip_error)?.to_string();

        if !name.to_lowercase().ends_with(".md") {
            skipped += 1;
            continue;
        }

        let stem = name.rsplit('/').next().unwrap_or(&name);
        let Some(iso) = date_from_stem(stem) else {
            skipped += 1;
            continue;
        };

        let mut text = String::new();
        file.read_to_string(&mut text).map_err(io_error)?;

        match import_markdown(&iso, &text).await? {
            ImportOutcome::Imported => imported += 1,
            ImportOutcome::Conflict => conflicts += 1,
            ImportOutcome::Unchanged => {}
        }
    }

    Ok((imported, conflicts, skipped))
}

async fn import_markdown(iso: &str, text: &str) -> Result<ImportOutcome, JsValue> {
    match opfs::read_entry(iso).await? {
        Some(existing) if existing == text => Ok(ImportOutcome::Unchanged),
        Some(_) => {
            opfs::write_conflict(iso, text).await?;
            Ok(ImportOutcome::Conflict)
        }
        None => {
            opfs::write_entry(iso, text).await?;
            Ok(ImportOutcome::Imported)
        }
    }
}

async fn read_file_text(file: &File) -> Result<String, JsValue> {
    let text = JsFuture::from(file.text()).await?;
    Ok(text.as_string().unwrap_or_default())
}

async fn read_file_bytes(file: &File) -> Result<Vec<u8>, JsValue> {
    let buffer = JsFuture::from(file.array_buffer()).await?;
    Ok(Uint8Array::new(&buffer).to_vec())
}

fn date_from_stem(stem: &str) -> Option<String> {
    let stem = stem.strip_suffix(".md").unwrap_or(stem);

    if stem.len() != 10 {
        return None;
    }

    let valid = stem.chars().enumerate().all(|(index, ch)| {
        if index == 4 || index == 7 {
            ch == '-'
        } else {
            ch.is_ascii_digit()
        }
    });

    if valid && parse_date(stem).is_some() {
        Some(stem.to_string())
    } else {
        None
    }
}

fn download_bytes(document: &Document, bytes: &[u8], filename: &str) -> Result<(), JsValue> {
    let parts = js_sys::Array::new();
    parts.push(&Uint8Array::from(bytes));
    let parts: JsValue = parts.into();
    let blob = Blob::new_with_u8_array_sequence(&parts)?;
    let url = Url::create_object_url_with_blob(&blob)?;
    let anchor: HtmlAnchorElement = document.create_element("a")?.dyn_into()?;

    anchor.set_href(&url);
    anchor.set_download(filename);
    document
        .body()
        .ok_or_else(|| JsValue::from_str("no body"))?
        .append_child(anchor.as_ref())?;
    anchor.click();
    anchor.remove();
    Url::revoke_object_url(&url)?;

    Ok(())
}

fn zip_error(error: zip::result::ZipError) -> JsValue {
    JsValue::from_str(&format!("zip: {error}"))
}

fn io_error(error: std::io::Error) -> JsValue {
    JsValue::from_str(&format!("io: {error}"))
}

fn record_history(state: &SharedState) {
    let source = markdown::source_from_editor(&state.borrow().editor);
    let mut borrowed = state.borrow_mut();

    if source == borrowed.last_snapshot {
        return;
    }

    let snapshot = borrowed.last_snapshot.clone();
    borrowed.undo_stack.push(snapshot);

    if borrowed.undo_stack.len() > 80 {
        borrowed.undo_stack.remove(0);
    }

    borrowed.last_snapshot = source;
    borrowed.redo_stack.clear();
}

fn push_current_history(state: &SharedState) {
    let current = markdown::source_from_editor(&state.borrow().editor);
    let mut borrowed = state.borrow_mut();

    if current == borrowed.last_snapshot {
        return;
    }

    borrowed.undo_stack.push(current.clone());

    if borrowed.undo_stack.len() > 80 {
        borrowed.undo_stack.remove(0);
    }

    borrowed.last_snapshot = current;
    borrowed.redo_stack.clear();
}

fn sync_snapshot(state: &SharedState) {
    let current = markdown::source_from_editor(&state.borrow().editor);
    state.borrow_mut().last_snapshot = current;
}

fn schedule_history(state: SharedState) {
    let next_state = state.clone();
    let mut state = state.borrow_mut();

    state.history_timeout = None;
    state.history_timeout = Some(Timeout::new(350, move || {
        record_history(&next_state);
    }));
}

fn undo(state: SharedState, document: &Document) {
    let current = markdown::source_from_editor(&state.borrow().editor);
    let mut borrowed = state.borrow_mut();
    let Some(previous) = borrowed.undo_stack.pop() else {
        return;
    };

    if current != previous {
        borrowed.redo_stack.push(current);
    }

    borrowed.last_snapshot = previous.clone();
    borrowed.history_timeout = None;
    drop(borrowed);

    restore_source(&state, document, &previous);
}

fn redo(state: SharedState, document: &Document) {
    let current = markdown::source_from_editor(&state.borrow().editor);
    let mut borrowed = state.borrow_mut();
    let Some(next) = borrowed.redo_stack.pop() else {
        return;
    };

    borrowed.undo_stack.push(current);
    borrowed.last_snapshot = next.clone();
    borrowed.history_timeout = None;
    drop(borrowed);

    restore_source(&state, document, &next);
}

fn restore_source(state: &SharedState, document: &Document, source: &str) {
    let editor = state.borrow().editor.clone();
    markdown::render_document(&editor, source);

    if let Ok(Some(line)) = editor.query_selector(".line:last-child") {
        line.class_list().add_1("active").ok();
        place_caret(document, &line, false);
    }

    editor.focus().ok();
    schedule_save(state.clone());
}

fn handle_paste(state: SharedState, document: &Document, event: Event) {
    let Ok(event) = event.dyn_into::<ClipboardEvent>() else {
        return;
    };

    let Some(data) = event.clipboard_data() else {
        return;
    };

    let Ok(text) = data.get_data("text/plain") else {
        return;
    };

    if text.is_empty() {
        return;
    }

    event.prevent_default();

    let Some(line) = ensure_active_line(document) else {
        return;
    };

    push_current_history(&state);

    let source = line.text_content().unwrap_or_default();
    let offset = caret_offset(document, &line).unwrap_or_else(|| utf16_len(&source));
    let (before, after) = split_at_utf16(&source, offset);
    let normalized = normalize_newlines(&text);
    let parts: Vec<&str> = normalized.split('\n').collect();

    if parts.len() == 1 {
        let next = format!("{before}{}{after}", parts[0]);
        markdown::render_line_into(&line, &next, true);
        place_caret_at_offset(document, &line, utf16_len(before) + utf16_len(parts[0]));
        sync_snapshot(&state);
        schedule_save(state);
        return;
    }

    let first = format!("{before}{}", parts[0]);
    markdown::render_line_into(&line, &first, false);

    let mut anchor = line;

    for part in &parts[1..parts.len() - 1] {
        if let Some(new_line) = insert_line_after(document, &anchor, part, false) {
            anchor = new_line;
        }
    }

    let last_source = format!("{}{after}", parts[parts.len() - 1]);

    if let Some(last_line) = insert_line_after(document, &anchor, &last_source, true) {
        last_line.class_list().add_1("active").ok();
        place_caret_at_offset(document, &last_line, utf16_len(parts[parts.len() - 1]));
    }

    sync_snapshot(&state);
    schedule_save(state);
}

fn ensure_active_line(document: &Document) -> Option<Element> {
    if let Some(line) = active_line(document) {
        return Some(line);
    }

    let editor = document.query_selector("#entry-editor").ok().flatten()?;
    let first = editor.first_element_child()?;
    first.class_list().add_1("active").ok()?;
    place_caret(document, &first, true);
    Some(first)
}

fn insert_line_after(
    document: &Document,
    line: &Element,
    source: &str,
    active: bool,
) -> Option<Element> {
    let new_line = document.create_element("div").ok()?;
    markdown::render_line_into(&new_line, source, active);

    let parent = line.parent_node()?;
    let reference = line.next_sibling();
    parent
        .insert_before(new_line.as_ref(), reference.as_ref())
        .ok()?;

    Some(new_line)
}

fn place_caret_at_offset(document: &Document, line: &Element, offset: usize) {
    let mut remaining = offset;

    if place_in_node(document, line.as_ref(), &mut remaining) {
        return;
    }

    place_caret(document, line, false);
}

fn place_in_node(document: &Document, node: &Node, remaining: &mut usize) -> bool {
    if node.node_type() == Node::TEXT_NODE {
        let length = utf16_len(&node.text_content().unwrap_or_default());

        if *remaining <= length {
            let Ok(range) = document.create_range() else {
                return false;
            };

            if range.set_start(node, *remaining as u32).is_err() {
                return false;
            }

            range.collapse_with_to_start(true);

            let Ok(Some(selection)) = document.get_selection() else {
                return false;
            };

            selection.remove_all_ranges().ok();
            selection.add_range(&range).ok();

            return true;
        }

        *remaining -= length;
    } else {
        let mut child = node.first_child();

        while let Some(current) = child {
            if place_in_node(document, &current, remaining) {
                return true;
            }

            child = current.next_sibling();
        }
    }

    false
}

fn normalize_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn update_active_line(state: SharedState, document: &Document) {
    let editor = state.borrow().editor.clone();
    let new_line = active_line(document);
    let old_line = editor.query_selector(".line.active").ok().flatten();

    if old_line == new_line {
        return;
    }

    if let Some(old) = old_line {
        let source = old.text_content().unwrap_or_default();
        markdown::render_line_into(&old, &source, false);
    }

    if let Some(new) = new_line {
        new.class_list().add_1("active").ok();
    }
}

fn split_line(state: SharedState, document: &Document) {
    let Some(line) = active_line(document) else {
        return;
    };

    push_current_history(&state);

    let source = line.text_content().unwrap_or_default();
    let offset = caret_offset(document, &line).unwrap_or_else(|| utf16_len(&source));
    let (before, after) = split_at_utf16(&source, offset);

    markdown::render_line_into(&line, before, false);

    let Ok(new_line) = document.create_element("div") else {
        return;
    };

    markdown::render_line_into(&new_line, after, true);

    let Some(parent) = line.parent_node() else {
        return;
    };

    let reference = line.next_sibling();
    if parent
        .insert_before(new_line.as_ref(), reference.as_ref())
        .is_err()
    {
        return;
    }

    place_caret(document, &new_line, true);
    sync_snapshot(&state);
    schedule_save(state);
}

fn merge_line(state: SharedState, document: &Document) -> bool {
    let Some(line) = active_line(document) else {
        return false;
    };

    if caret_offset(document, &line) != Some(0) {
        return false;
    }

    let Some(previous) = line.previous_element_sibling() else {
        return false;
    };

    push_current_history(&state);

    let mut source = previous.text_content().unwrap_or_default();
    source.push_str(&line.text_content().unwrap_or_default());

    markdown::render_line_into(&previous, &source, true);
    line.remove();
    place_caret(document, &previous, false);
    sync_snapshot(&state);
    schedule_save(state);
    true
}

fn active_line(document: &Document) -> Option<Element> {
    let selection = document.get_selection().ok()??;
    let node = selection.anchor_node()?;

    markdown::line_of(&node)
}

fn caret_offset(document: &Document, line: &Element) -> Option<usize> {
    let selection = document.get_selection().ok()??;
    let anchor = selection.anchor_node()?;
    let offset = selection.anchor_offset();
    let mut total = 0;
    let mut found = false;

    walk_caret(
        line.as_ref(),
        &anchor,
        offset,
        &mut total,
        &mut found,
    );

    found.then_some(total)
}

fn walk_caret(
    node: &Node,
    target: &Node,
    target_offset: u32,
    total: &mut usize,
    found: &mut bool,
) {
    if *found {
        return;
    }

    if node.is_same_node(Some(target)) {
        if node.node_type() == Node::TEXT_NODE {
            *total += target_offset as usize;
        } else {
            let mut child = node.first_child();
            let mut index = 0;

            while let Some(current) = child {
                if index >= target_offset {
                    break;
                }

                *total += utf16_len(&current.text_content().unwrap_or_default());
                index += 1;
                child = current.next_sibling();
            }
        }

        *found = true;
        return;
    }

    let mut child = node.first_child();

    while let Some(current) = child {
        walk_caret(&current, target, target_offset, total, found);

        if *found {
            return;
        }

        if current.node_type() == Node::TEXT_NODE {
            *total += utf16_len(&current.text_content().unwrap_or_default());
        }

        child = current.next_sibling();
    }
}

fn place_caret(document: &Document, line: &Element, at_start: bool) {
    let Ok(range) = document.create_range() else {
        return;
    };

    if range.select_node_contents(line.as_ref()).is_err() {
        return;
    }

    range.collapse_with_to_start(at_start);

    let Ok(Some(selection)) = document.get_selection() else {
        return;
    };

    selection.remove_all_ranges().ok();
    selection.add_range(&range).ok();
}

fn split_at_utf16(source: &str, offset: usize) -> (&str, &str) {
    let mut units = 0;

    for (index, ch) in source.char_indices() {
        if units >= offset {
            return source.split_at(index);
        }

        units += ch.len_utf16();
    }

    (source, "")
}

fn utf16_len(value: &str) -> usize {
    value.encode_utf16().count()
}

async fn load_entry(state: SharedState, iso: &str) -> Result<(), JsValue> {
    let result = opfs::read_entry(iso).await;

    match result {
        Ok(Some(text)) => {
            markdown::render_document(&state.borrow().editor, &text);

            {
                let mut state = state.borrow_mut();
                state.undo_stack.clear();
                state.redo_stack.clear();
                state.last_snapshot = text.clone();
                state.base_text = text.clone();
                state.history_timeout = None;
            }

            let meta = parse_date(iso).map(|date| parse_entry(&text, date));
            let words = meta.as_ref().map_or(0, |meta| meta.words);
            let tags = meta
                .as_ref()
                .map_or_else(String::new, |meta| render::editor_tags(&meta.tags));

            state.borrow().editor_tags.set_inner_html(&tags);
            state.borrow_mut().has_file = true;
            set_status(&state, &format!("Loaded · {words} words"));
        }
        Ok(None) => {
            markdown::render_document(&state.borrow().editor, "");
            state.borrow().editor_tags.set_inner_html("");
            {
                let mut state = state.borrow_mut();
                state.has_file = false;
                state.base_text.clear();
                state.undo_stack.clear();
                state.redo_stack.clear();
                state.last_snapshot.clear();
                state.history_timeout = None;
            }
            set_status(&state, "New entry — not saved yet");
        }
        Err(error) => {
            web_sys::console::error_1(&error);
            set_status(&state, "Could not open local storage");
        }
    }

    Ok(())
}

async fn save_entry(state: SharedState) -> Result<(), JsValue> {
    let (iso, text, has_file, editor, base_text) = {
        let state = state.borrow();
        (
            state.selected_iso.clone(),
            markdown::source_from_editor(&state.editor),
            state.has_file,
            state.editor.clone(),
            state.base_text.clone(),
        )
    };

    if text.trim().is_empty() && !has_file {
        set_status(&state, "New entry — not saved yet");
        return Ok(());
    }

    if state.borrow().saving {
        return Ok(());
    }

    state.borrow_mut().saving = true;
    set_status(&state, "Saving…");

    if has_file {
        match opfs::read_entry(&iso).await {
            Ok(Some(remote)) => {
                if remote != base_text && remote != text {
                    let conflict = opfs::write_conflict(&iso, &text).await;
                    state.borrow_mut().saving = false;

                    match conflict {
                        Ok(name) => {
                            markdown::render_document(&editor, &remote);

                            let meta = parse_date(&iso).map(|date| parse_entry(&remote, date));
                            let tags = meta.as_ref().map_or_else(String::new, |meta| {
                                render::editor_tags(&meta.tags)
                            });

                            state.borrow().editor_tags.set_inner_html(&tags);
                            {
                                let mut state = state.borrow_mut();
                                state.base_text = remote.clone();
                                state.last_snapshot = remote;
                                state.undo_stack.clear();
                                state.redo_stack.clear();
                                state.has_file = true;
                            }
                            set_status(&state, &format!("Conflict copy saved as {name}"));
                            refresh_if_no_inline(state.clone()).await?;
                        }
                        Err(error) => {
                            web_sys::console::error_1(&error);
                            set_status(&state, "Conflict copy failed");
                        }
                    }

                    return Ok(());
                }

                if remote == text {
                    state.borrow_mut().saving = false;
                    state.borrow_mut().base_text = text.clone();

                    let words = parse_date(&iso)
                        .map(|date| parse_entry(&text, date).words)
                        .unwrap_or(0);

                    set_status(&state, &format!("Saved locally · {words} words"));
                    refresh_if_no_inline(state.clone()).await?;
                    return Ok(());
                }
            }
            Ok(None) => {
                let conflict = opfs::write_conflict(&iso, &text).await;
                state.borrow_mut().saving = false;

                match conflict {
                    Ok(name) => {
                        markdown::render_document(&editor, "");
                        state.borrow().editor_tags.set_inner_html("");
                        {
                            let mut state = state.borrow_mut();
                            state.base_text.clear();
                            state.has_file = false;
                            state.last_snapshot.clear();
                            state.undo_stack.clear();
                            state.redo_stack.clear();
                        }
                        set_status(
                            &state,
                            &format!("Conflict copy saved as {name} — entry was deleted"),
                        );
                        refresh_if_no_inline(state.clone()).await?;
                    }
                    Err(error) => {
                        web_sys::console::error_1(&error);
                        set_status(&state, "Conflict copy failed");
                    }
                }

                return Ok(());
            }
            Err(error) => {
                state.borrow_mut().saving = false;
                web_sys::console::error_1(&error);
                set_status(&state, "Could not check the file");
                return Ok(());
            }
        }
    }

    let result = opfs::write_entry(&iso, &text).await;
    state.borrow_mut().saving = false;

    match result {
        Ok(()) => {
            let meta = parse_date(&iso).map(|date| parse_entry(&text, date));
            let words = meta.as_ref().map_or(0, |meta| meta.words);
            let tags = meta
                .as_ref()
                .map_or_else(String::new, |meta| render::editor_tags(&meta.tags));

            state.borrow().editor_tags.set_inner_html(&tags);
            {
                let mut state = state.borrow_mut();
                state.has_file = true;
                state.base_text = text.clone();
            }
            set_status(&state, &format!("Saved locally · {words} words"));
            refresh_if_no_inline(state.clone()).await?;

            if markdown::source_from_editor(&editor) != text {
                schedule_save(state.clone());
            }
        }
        Err(error) => {
            web_sys::console::error_1(&error);
            set_status(&state, "Save failed");
        }
    }

    Ok(())
}

async fn refresh(state: SharedState) -> Result<(), JsValue> {
    match opfs::list_entries().await {
        Ok(raw_entries) => {
            let mut entries = Vec::new();

            for (iso, text) in raw_entries {
                if let Some(date) = parse_date(&iso) {
                    let meta = parse_entry(&text, date);
                    entries.push(EntryRecord { iso, text, meta });
                }
            }

            entries.sort_by(|left, right| right.iso.cmp(&left.iso));

            let mut borrowed = state.borrow_mut();
            if let Some(tag) = &borrowed.active_tag {
                let still_exists = entries
                    .iter()
                    .any(|entry| entry.meta.tags.iter().any(|value| value == tag));

                if !still_exists {
                    borrowed.active_tag = None;
                }
            }

            borrowed.entries = entries;
            drop(borrowed);
            render_ui(state.clone());
        }
        Err(error) => {
            web_sys::console::error_1(&error);
            set_status(&state, "Could not scan journal");
        }
    }

    Ok(())
}

async fn refresh_if_no_inline(state: SharedState) -> Result<(), JsValue> {
    if state.borrow().inline_iso.is_none() {
        refresh(state).await?;
    }

    Ok(())
}

fn render_ui(state: SharedState) {
    let (timeline, tags, calendar, filter_visible, filter_label) = {
        let state = state.borrow();

        (
            render::timeline(
                &state.entries,
                &state.selected_iso,
                state.active_tag.as_deref(),
                state.timeline_limit,
            ),
            render::tags(&state.entries, state.active_tag.as_deref()),
            render::calendar(
                state.calendar_year,
                state.calendar_month,
                &state.selected_iso,
                &state.today_iso,
                &state.entries,
            ),
            state.active_tag.is_some(),
            state
                .active_tag
                .as_ref()
                .map(|tag| format!("#{tag}"))
                .unwrap_or_default(),
        )
    };

    let state = state.borrow();
    state.timeline.set_inner_html(&timeline);
    state.tag_list.set_inner_html(&tags);
    state.calendar.set_inner_html(&calendar);

    if filter_visible {
        state.active_filter.remove_attribute("hidden").ok();
    } else {
        state.active_filter.set_attribute("hidden", "true").ok();
    }

    state
        .active_filter_label
        .set_text_content(Some(&filter_label));
}

fn set_active_tag(state: SharedState, tag: Option<String>) {
    {
        let mut state = state.borrow_mut();
        state.active_tag = tag;
        state.timeline_limit = 30;
    }
    render_ui(state);
}

fn open_entry(state: SharedState, iso: String) {
    if state.borrow().selected_iso == iso {
        spawn_result(finish_inline_edit(state.clone(), true));

        let editor = state.borrow().editor.clone();
        editor.focus().ok();
        editor.scroll_into_view_with_bool(true);
        return;
    }

    let timeline = state.borrow().timeline.clone();
    let selector = format!("a[data-iso=\"{iso}\"]");
    let mut link = timeline.query_selector(&selector).ok().flatten();

    if link.is_none() {
        state.borrow_mut().active_tag = None;
        render_ui(state.clone());
        link = timeline.query_selector(&selector).ok().flatten();
    }

    if let Some(link) = link {
        link.scroll_into_view_with_bool(true);
        spawn_result(start_inline_edit(state, iso));
    } else {
        spawn_result(select_date(state, iso));
    }
}

async fn start_inline_edit(state: SharedState, iso: String) -> Result<(), JsValue> {
    if state.borrow().inline_iso.as_deref() == Some(iso.as_str()) {
        return Ok(());
    }

    finish_inline_edit(state.clone(), true).await?;

    if state.borrow().inline_iso.is_some() {
        return Ok(());
    }

    save_entry(state.clone()).await?;

    let (text, timeline) = {
        let state = state.borrow();
        let text = state
            .entries
            .iter()
            .find(|entry| entry.iso == iso)
            .map(|entry| entry.text.clone())
            .unwrap_or_default();

        (text, state.timeline.clone())
    };

    let Some(link) = timeline.query_selector(&format!("a[data-iso=\"{iso}\"]"))? else {
        return Ok(());
    };

    let document = timeline
        .owner_document()
        .ok_or_else(|| JsValue::from_str("no document"))?;
    let container = document.create_element("div")?;
    container.set_inner_html(&render::inline_editor(&text));

    let Some(parent) = link.parent_node() else {
        return Ok(());
    };

    let reference = link.next_sibling();
    parent.insert_before(container.as_ref(), reference.as_ref())?;
    link.set_attribute("hidden", "true")?;

    let textarea: HtmlTextAreaElement = container
        .query_selector("textarea")?
        .ok_or_else(|| JsValue::from_str("missing inline textarea"))?
        .dyn_into()?;
    let status = container
        .query_selector(".inline-status")?
        .ok_or_else(|| JsValue::from_str("missing inline status"))?;
    let done = container
        .query_selector(".inline-done")?
        .ok_or_else(|| JsValue::from_str("missing inline done"))?;

    let input_state = state.clone();
    listen(
        textarea.as_ref(),
        "input",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            schedule_inline_save(input_state.clone());
        }),
    )?;

    let done_state = state.clone();
    listen(
        done.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            spawn_result(finish_inline_edit(done_state.clone(), true));
        }),
    )?;

    let key_state = state.clone();
    listen(
        textarea.as_ref(),
        "keydown",
        Closure::<dyn FnMut(Event)>::new(move |event| {
            handle_inline_keydown(key_state.clone(), event);
        }),
    )?;

    let length = textarea.value().encode_utf16().count() as u32;
    textarea.set_selection_range(length, length).ok();

    {
        let mut state = state.borrow_mut();
        state.inline_iso = Some(iso);
        state.inline_base = text;
        state.inline_link = Some(link);
        state.inline_container = Some(container);
        state.inline_textarea = Some(textarea.clone());
        state.inline_status = Some(status);
        state.inline_saving = false;
        state.inline_save_timeout = None;
    }

    textarea.focus().ok();
    Ok(())
}

async fn finish_inline_edit(state: SharedState, save: bool) -> Result<(), JsValue> {
    if state.borrow().inline_iso.is_none() {
        return Ok(());
    }

    if save && !save_inline(state.clone()).await? {
        return Ok(());
    }

    let (container, link) = {
        let mut state = state.borrow_mut();
        let container = state.inline_container.take();
        let link = state.inline_link.take();

        state.inline_iso = None;
        state.inline_base.clear();
        state.inline_textarea = None;
        state.inline_status = None;
        state.inline_saving = false;
        state.inline_save_timeout = None;

        (container, link)
    };

    if let Some(container) = container {
        container.remove();
    }

    if let Some(link) = link {
        link.remove_attribute("hidden").ok();
    }

    refresh(state).await?;
    Ok(())
}

async fn save_inline(state: SharedState) -> Result<bool, JsValue> {
    let (iso, text, base, textarea, status) = {
        let state = state.borrow();
        let Some(iso) = state.inline_iso.clone() else {
            return Ok(true);
        };
        let Some(textarea) = state.inline_textarea.clone() else {
            return Ok(true);
        };

        (
            iso,
            textarea.value(),
            state.inline_base.clone(),
            textarea,
            state.inline_status.clone(),
        )
    };

    if text == base {
        set_inline_status(status.as_ref(), "Saved");
        return Ok(true);
    }

    if state.borrow().inline_saving {
        return Ok(false);
    }

    state.borrow_mut().inline_saving = true;
    set_inline_status(status.as_ref(), "Saving…");

    let result = opfs::read_entry(&iso).await;

    let outcome = match result {
        Ok(Some(remote)) if remote != base && remote != text => {
            let conflict = opfs::write_conflict(&iso, &text).await;

            match conflict {
                Ok(name) => {
                    textarea.set_value(&remote);
                    state.borrow_mut().inline_base = remote;
                    set_inline_status(status.as_ref(), &format!("Conflict copy saved as {name}"));
                    true
                }
                Err(error) => {
                    web_sys::console::error_1(&error);
                    set_inline_status(status.as_ref(), "Conflict copy failed");
                    false
                }
            }
        }
        Ok(Some(remote)) if remote == text => {
            state.borrow_mut().inline_base = text;
            set_inline_status(status.as_ref(), "Saved locally");
            true
        }
        Ok(None) => {
            let conflict = opfs::write_conflict(&iso, &text).await;

            match conflict {
                Ok(name) => {
                    textarea.set_value("");
                    state.borrow_mut().inline_base.clear();
                    set_inline_status(
                        status.as_ref(),
                        &format!("Conflict copy saved as {name} — entry was deleted"),
                    );
                    true
                }
                Err(error) => {
                    web_sys::console::error_1(&error);
                    set_inline_status(status.as_ref(), "Conflict copy failed");
                    false
                }
            }
        }
        Ok(Some(_)) => match opfs::write_entry(&iso, &text).await {
            Ok(()) => {
                state.borrow_mut().inline_base = text;
                set_inline_status(status.as_ref(), "Saved locally");
                true
            }
            Err(error) => {
                web_sys::console::error_1(&error);
                set_inline_status(status.as_ref(), "Save failed");
                false
            }
        },
        Err(error) => {
            web_sys::console::error_1(&error);
            set_inline_status(status.as_ref(), "Could not check the file");
            false
        }
    };

    state.borrow_mut().inline_saving = false;
    Ok(outcome)
}

fn schedule_inline_save(state: SharedState) {
    if let Some(status) = state.borrow().inline_status.as_ref() {
        status.set_text_content(Some("Unsaved changes"));
    }

    let next_state = state.clone();
    let mut state = state.borrow_mut();

    state.inline_save_timeout = None;
    state.inline_save_timeout = Some(Timeout::new(400, move || {
        spawn_result(async move { save_inline(next_state).await.map(|_| ()) });
    }));
}

fn set_inline_status(status: Option<&Element>, text: &str) {
    if let Some(status) = status {
        status.set_text_content(Some(text));
    }
}

fn handle_inline_keydown(state: SharedState, event: Event) {
    let Ok(event) = event.dyn_into::<KeyboardEvent>() else {
        return;
    };

    if event.is_composing() {
        return;
    }

    let modifier = event.meta_key() || event.ctrl_key();

    if event.key() == "Escape" || (modifier && event.key() == "Enter") {
        event.prevent_default();
        spawn_result(finish_inline_edit(state, true));
    }
}

async fn select_date(state: SharedState, iso: String) -> Result<(), JsValue> {
    finish_inline_edit(state.clone(), true).await?;

    if state.borrow().selected_iso == iso {
        return Ok(());
    }

    save_entry(state.clone()).await?;

    {
        let mut state = state.borrow_mut();
        state.selected_iso = iso.clone();
        state.has_file = false;
        state.timeline_limit = 30;
        let (year, month) = month_parts(&iso);
        state.calendar_year = year;
        state.calendar_month = month;
    }

    update_labels(&state);
    load_entry(state.clone(), &iso).await?;
    refresh(state.clone()).await?;

    let editor = state.borrow().editor.clone();
    editor.focus().ok();
    editor.scroll_into_view_with_bool(true);

    Ok(())
}

fn shift_date(state: SharedState, days: i64) {
    let iso = state.borrow().selected_iso.clone();
    let Some(date) = parse_date(&iso) else {
        return;
    };

    let next = date + Duration::days(days);
    let next_iso = next.to_string();
    let today = state.borrow().today_iso.clone();

    if next_iso > today {
        return;
    }

    spawn_result(select_date(state, next_iso));
}

fn shift_month(state: SharedState, delta: i32) {
    let (year, month) = {
        let state = state.borrow();
        (state.calendar_year, state.calendar_month as i32)
    };

    let mut next_year = year;
    let mut next_month = month + delta;

    while next_month < 1 {
        next_month += 12;
        next_year -= 1;
    }

    while next_month > 12 {
        next_month -= 12;
        next_year += 1;
    }

    {
        let mut state = state.borrow_mut();
        state.calendar_year = next_year;
        state.calendar_month = next_month as u32;
    }

    render_ui(state);
}

fn update_labels(state: &SharedState) {
    let (selected_iso, today_iso) = {
        let state = state.borrow();
        (state.selected_iso.clone(), state.today_iso.clone())
    };

    let Some(date) = parse_date(&selected_iso) else {
        return;
    };

    let state = state.borrow();
    state
        .day_label
        .set_text_content(Some(&date.day().to_string()));
    state.weekday_label.set_text_content(Some(if selected_iso == today_iso {
        "Today"
    } else {
        weekday_name(date)
    }));
    state.month_label.set_text_content(Some(&month_year(date)));

    if selected_iso == today_iso {
        state.today_button.set_attribute("hidden", "true").ok();
        state
            .next_day_button
            .set_attribute("disabled", "true")
            .ok();
    } else {
        state.today_button.remove_attribute("hidden").ok();
        state.next_day_button.remove_attribute("disabled").ok();
    }
}

fn schedule_save(state: SharedState) {
    set_status(&state, "Unsaved changes");

    let next_state = state.clone();
    let mut state = state.borrow_mut();

    state.save_timeout = None;
    state.save_timeout = Some(Timeout::new(400, move || {
        spawn_result(save_entry(next_state.clone()));
    }));
}

fn set_status(state: &SharedState, text: &str) {
    state.borrow().status.set_text_content(Some(text));
}

fn today_iso() -> String {
    let date = js_sys::Date::new_0();

    format!(
        "{:04}-{:02}-{:02}",
        date.get_full_year(),
        date.get_month() + 1,
        date.get_date()
    )
}

fn month_parts(iso: &str) -> (i32, u32) {
    let year = iso[0..4].parse::<i32>().unwrap_or(2026);
    let month = iso[5..7].parse::<u32>().unwrap_or(10);

    (year, month)
}

fn query<T: JsCast>(document: &Document, selector: &str) -> Result<T, JsValue> {
    document
        .query_selector(selector)?
        .ok_or_else(|| JsValue::from_str(&format!("missing {selector}")))?
        .dyn_into::<T>()
        .map_err(|_| JsValue::from_str(&format!("wrong element for {selector}")))
}

fn element(document: &Document, selector: &str) -> Result<Element, JsValue> {
    document
        .query_selector(selector)?
        .ok_or_else(|| JsValue::from_str(&format!("missing {selector}")))
}

fn target_element(event: &Event) -> Option<Element> {
    event.target()?.dyn_into::<Element>().ok()
}
