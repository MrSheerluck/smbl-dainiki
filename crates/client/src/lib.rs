mod markdown;
mod opfs;
mod render;
mod sync;

use std::{
    cell::RefCell,
    collections::HashMap,
    future::Future,
    io::{Cursor, Read, Write},
    rc::Rc,
};

use dainiki_core::{
    month_year, parse_date, parse_entry,
    sync_crypto::{
        ManifestEntry, SyncManifest, bytes_hash, content_hash, create_sync_keys, decrypt_entry,
        decrypt_manifest, encrypt_entry, encrypt_manifest, unlock_with_passphrase,
        unlock_with_recovery_key,
    },
    weekday_name,
};
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
    welcome_overlay: Element,
    storage_status: Element,
    day_label: Element,
    weekday_label: Element,
    month_label: Element,
    today_button: Element,
    next_day_button: Element,
    save_timeout: Option<Timeout>,
    history_timeout: Option<Timeout>,
    sync_overlay: Element,
    sync_content: Element,
    sync_status: Option<sync::SyncStatus>,
    sync_signed_out: bool,
    sync_keys: Option<sync::SyncKeys>,
    sync_master_key: Option<[u8; 32]>,
    sync_recovery: Option<String>,
    sync_base: Option<SyncManifest>,
    sync_message: Option<String>,
    sync_busy: bool,
    sync_auto_timeout: Option<Timeout>,
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
        welcome_overlay: element(&document, "#welcome-overlay")?,
        storage_status: element(&document, "#storage-status")?,
        day_label: element(&document, "#today-day")?,
        weekday_label: element(&document, "#today-weekday")?,
        month_label: element(&document, "#today-month")?,
        today_button: element(&document, "#today-button")?,
        next_day_button: element(&document, "#next-day")?,
        save_timeout: None,
        history_timeout: None,
        sync_overlay: element(&document, "#sync-overlay")?,
        sync_content: element(&document, "#sync-content")?,
        sync_status: None,
        sync_signed_out: false,
        sync_keys: None,
        sync_master_key: None,
        sync_recovery: None,
        sync_base: None,
        sync_message: None,
        sync_busy: false,
        sync_auto_timeout: None,
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
    refresh(state.clone()).await?;
    maybe_show_welcome(&state);
    Ok(())
}

fn start_writing(state: SharedState) {
    state
        .borrow()
        .welcome_overlay
        .set_attribute("hidden", "true")
        .ok();

    if let Some(storage) = local_storage() {
        storage.set_item("dainiki-onboarded", "true").ok();
    }

    let editor = state.borrow().editor.clone();
    editor.focus().ok();
    editor.scroll_into_view_with_bool(true);

    spawn_result(request_persistent_storage(state));
}

async fn request_persistent_storage(state: SharedState) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let storage = window.navigator().storage();
    let granted = JsFuture::from(storage.persist()?)
        .await?
        .as_bool()
        .unwrap_or(false);

    let message = if granted {
        "Storage protected"
    } else {
        "Storage may be cleared by the browser"
    };

    state
        .borrow()
        .storage_status
        .set_text_content(Some(message));
    Ok(())
}

fn maybe_show_welcome(state: &SharedState) {
    let has_entries = !state.borrow().entries.is_empty();
    let onboarded = local_storage()
        .and_then(|storage| storage.get_item("dainiki-onboarded").ok().flatten())
        .as_deref()
        == Some("true");

    if !has_entries && !onboarded {
        state
            .borrow()
            .welcome_overlay
            .remove_attribute("hidden")
            .ok();
    }
}

fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
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

    let start_writing_state = state.clone();
    listen(
        element(document, "#start-writing")?.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            start_writing(start_writing_state.clone());
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

    let open_sync_state = state.clone();
    listen(
        element(document, "#open-sync")?.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            open_sync(open_sync_state.clone());
        }),
    )?;

    let close_sync_state = state.clone();
    listen(
        element(document, "#close-sync")?.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            close_sync(close_sync_state.clone());
        }),
    )?;

    let sync_backdrop_state = state.clone();
    listen(
        state.borrow().sync_overlay.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |event| {
            let Some(target) = target_element(&event) else {
                return;
            };

            if target.id() == "sync-overlay" {
                close_sync(sync_backdrop_state.clone());
            }
        }),
    )?;

    let sync_action_state = state.clone();
    let sync_action_document = document.clone();
    listen(
        state.borrow().sync_content.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |event| {
            handle_sync_action(sync_action_state.clone(), &sync_action_document, event);
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

    let delete_entry_state = state.clone();
    listen(
        element(document, "#delete-entry")?.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            spawn_result(delete_current_entry(delete_entry_state.clone()));
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

fn open_sync(state: SharedState) {
    state
        .borrow()
        .sync_overlay
        .remove_attribute("hidden")
        .ok();
    render_sync_panel(state.clone());
    spawn_result(load_sync_status(state));
}

fn close_sync(state: SharedState) {
    state
        .borrow()
        .sync_overlay
        .set_attribute("hidden", "true")
        .ok();
}

async fn load_sync_status(state: SharedState) -> Result<(), JsValue> {
    match sync::fetch_status().await {
        Ok(status) => {
            let linked = status.linked;
            {
                let mut state = state.borrow_mut();
                state.sync_status = Some(status);
                state.sync_signed_out = false;
            }

            if linked && state.borrow().sync_keys.is_none() {
                if let Ok(keys) = sync::fetch_keys().await {
                    state.borrow_mut().sync_keys = Some(keys);
                }
            }
        }
        Err(sync::SyncError::Unauthorized) => {
            let mut state = state.borrow_mut();
            state.sync_status = None;
            state.sync_signed_out = true;
            state.sync_keys = None;
            state.sync_master_key = None;
        }
        Err(error) => {
            state.borrow_mut().sync_message = Some(error.message());
        }
    }

    render_sync_panel(state);
    Ok(())
}

fn handle_sync_action(state: SharedState, document: &Document, event: Event) {
    let Some(target) = target_element(&event) else {
        return;
    };
    let Ok(Some(button)) = target.closest("[data-sync-action]") else {
        return;
    };
    let Some(action) = button.get_attribute("data-sync-action") else {
        return;
    };

    match action.as_str() {
        "setup" => {
            let passphrase = input_value(document, "#sync-passphrase");
            let confirm = input_value(document, "#sync-confirm");
            spawn_result(setup_sync(state, passphrase, confirm));
        }
        "unlock" => {
            let passphrase = input_value(document, "#sync-passphrase");
            spawn_result(unlock_sync(state, passphrase));
        }
        "unlock-recovery" => {
            let recovery = input_value(document, "#sync-recovery-input");
            spawn_result(unlock_recovery_sync(state, recovery));
        }
        "sync-now" => spawn_result(run_sync(state)),
        "lock" => lock_sync(state),
        _ => {}
    }
}

async fn setup_sync(
    state: SharedState,
    passphrase: String,
    confirm: String,
) -> Result<(), JsValue> {
    if passphrase.chars().count() < 12 {
        set_sync_message(&state, "Use at least 12 characters for the sync passphrase.");
        render_sync_panel(state);
        return Ok(());
    }

    if passphrase != confirm {
        set_sync_message(&state, "Those sync passphrases do not match.");
        render_sync_panel(state);
        return Ok(());
    }

    state.borrow_mut().sync_busy = true;
    set_sync_message(&state, "Deriving your encryption key…");
    render_sync_panel(state.clone());

    let keys = match create_sync_keys(&passphrase) {
        Ok(keys) => keys,
        Err(error) => {
            state.borrow_mut().sync_busy = false;
            set_sync_message(&state, &error.to_string());
            render_sync_panel(state);
            return Ok(());
        }
    };

    let response = match sync::create_keys(
        &keys.salt,
        &keys.wrapped_key,
        &keys.recovery_wrapped_key,
    )
    .await
    {
        Ok(response) => response,
        Err(error) => {
            state.borrow_mut().sync_busy = false;
            set_sync_message(&state, &error.message());
            render_sync_panel(state);
            return Ok(());
        }
    };

    {
        let mut state = state.borrow_mut();
        state.sync_busy = false;
        state.sync_keys = Some(response);
        state.sync_master_key = Some(keys.master_key);
        state.sync_recovery = Some(keys.recovery_key);
        state.sync_base = load_sync_base();
        if let Some(status) = &mut state.sync_status {
            status.linked = true;
        }
    }

    set_sync_message(
        &state,
        "Sync key created. Save the recovery key below, then run a sync.",
    );
    render_sync_panel(state);
    Ok(())
}

async fn unlock_sync(state: SharedState, passphrase: String) -> Result<(), JsValue> {
    state.borrow_mut().sync_busy = true;
    set_sync_message(&state, "Unlocking encrypted sync…");
    render_sync_panel(state.clone());

    let keys = match sync_keys(&state).await {
        Ok(keys) => keys,
        Err(message) => {
            state.borrow_mut().sync_busy = false;
            set_sync_message(&state, &message);
            render_sync_panel(state);
            return Ok(());
        }
    };

    let master = match unlock_with_passphrase(&passphrase, &keys.salt, &keys.wrapped_key) {
        Ok(master) => master,
        Err(_) => {
            state.borrow_mut().sync_busy = false;
            set_sync_message(&state, "That sync passphrase is incorrect.");
            render_sync_panel(state);
            return Ok(());
        }
    };

    {
        let mut state = state.borrow_mut();
        state.sync_busy = false;
        state.sync_keys = Some(keys);
        state.sync_master_key = Some(master);
        state.sync_recovery = None;
        state.sync_base = load_sync_base();
    }

    render_sync_panel(state.clone());
    run_sync(state).await
}

async fn unlock_recovery_sync(state: SharedState, recovery: String) -> Result<(), JsValue> {
    state.borrow_mut().sync_busy = true;
    set_sync_message(&state, "Unlocking encrypted sync…");
    render_sync_panel(state.clone());

    let keys = match sync_keys(&state).await {
        Ok(keys) => keys,
        Err(message) => {
            state.borrow_mut().sync_busy = false;
            set_sync_message(&state, &message);
            render_sync_panel(state);
            return Ok(());
        }
    };

    let master = match unlock_with_recovery_key(&recovery, &keys.recovery_wrapped_key) {
        Ok(master) => master,
        Err(_) => {
            state.borrow_mut().sync_busy = false;
            set_sync_message(&state, "That recovery key is not valid.");
            render_sync_panel(state);
            return Ok(());
        }
    };

    {
        let mut state = state.borrow_mut();
        state.sync_busy = false;
        state.sync_keys = Some(keys);
        state.sync_master_key = Some(master);
        state.sync_recovery = None;
        state.sync_base = load_sync_base();
    }

    render_sync_panel(state.clone());
    run_sync(state).await
}

fn lock_sync(state: SharedState) {
    let recovery = state.borrow().sync_recovery.is_some();
    {
        let mut state = state.borrow_mut();
        state.sync_master_key = None;
        state.sync_recovery = None;
        state.sync_message = Some(if recovery {
            "Sync locked. Make sure you saved the recovery key before closing.".to_string()
        } else {
            "Sync locked on this device.".to_string()
        });
    }
    render_sync_panel(state);
}

async fn sync_keys(state: &SharedState) -> Result<sync::SyncKeys, String> {
    if let Some(keys) = state.borrow().sync_keys.clone() {
        return Ok(keys);
    }

    let keys = sync::fetch_keys().await.map_err(|error| error.message())?;
    state.borrow_mut().sync_keys = Some(keys.clone());
    Ok(keys)
}

async fn run_sync(state: SharedState) -> Result<(), JsValue> {
    run_sync_mode(state, true).await
}

async fn run_sync_mode(state: SharedState, reload_editor: bool) -> Result<(), JsValue> {
    if state.borrow().sync_master_key.is_none() || state.borrow().sync_busy {
        return Ok(());
    }

    state.borrow_mut().sync_busy = true;
    set_sync_message(&state, "Syncing encrypted entries…");
    render_sync_panel(state.clone());

    let result = perform_sync(state.clone(), reload_editor).await;

    {
        let mut state = state.borrow_mut();
        state.sync_busy = false;
        state.sync_message = Some(match &result {
            Ok(message) => message.clone(),
            Err(message) => message.clone(),
        });
    }

    if let Ok(status) = sync::fetch_status().await {
        state.borrow_mut().sync_status = Some(status);
    }
    render_sync_panel(state);
    Ok(())
}

async fn perform_sync(state: SharedState, reload_editor: bool) -> Result<String, String> {
    if state.borrow().inline_iso.is_some() {
        finish_inline_edit(state.clone(), true)
            .await
            .map_err(js_message)?;
    }
    save_entry(state.clone()).await.map_err(js_message)?;

    let key = state
        .borrow()
        .sync_master_key
        .ok_or_else(|| "Sync is locked.".to_string())?;
    let local_pairs = opfs::list_entries().await.map_err(js_message)?;
    let mut local: HashMap<String, String> = local_pairs.into_iter().collect();

    let remote = sync::fetch_manifest().await.map_err(|error| error.message())?;
    let remote_revision = remote.as_ref().map(|manifest| manifest.revision).unwrap_or(0);
    let mut manifest = match &remote {
        Some(remote) => decrypt_manifest(&key, &remote.bytes).map_err(|error| error.to_string())?,
        None => SyncManifest {
            version: 1,
            updated_at: 0,
            entries: Vec::new(),
        },
    };
    let mut manifest_entries: HashMap<String, ManifestEntry> = manifest
        .entries
        .drain(..)
        .map(|entry| (entry.date.clone(), entry))
        .collect();
    let base = state
        .borrow()
        .sync_base
        .clone()
        .or_else(load_sync_base);
    let base_entries: HashMap<String, ManifestEntry> = base
        .as_ref()
        .map(|manifest| {
            manifest
                .entries
                .iter()
                .cloned()
                .map(|entry| (entry.date.clone(), entry))
                .collect()
        })
        .unwrap_or_default();

    let mut uploaded = 0usize;
    let mut downloaded = 0usize;
    let mut conflicts = 0usize;
    let mut deleted = 0usize;

    let tombstones: Vec<ManifestEntry> = manifest_entries
        .values()
        .filter(|entry| entry.deleted)
        .cloned()
        .collect();

    for tombstone in tombstones {
        let created_after_delete = base_entries
            .get(&tombstone.date)
            .is_some_and(|entry| entry.deleted);

        if created_after_delete {
            continue;
        }

        if let Some(text) = local.remove(&tombstone.date) {
            let local_hash = content_hash(&text);
            let unchanged = base_entries
                .get(&tombstone.date)
                .is_some_and(|entry| entry.hash == local_hash);

            if !unchanged {
                opfs::write_conflict(&tombstone.date, &text)
                    .await
                    .map_err(js_message)?;
                conflicts += 1;
            }

            opfs::delete_entry(&tombstone.date)
                .await
                .map_err(js_message)?;
            deleted += 1;
        }
    }

    let pending_deletions = load_sync_deletions();
    let mut remaining_deletions = Vec::new();

    for (index, iso) in pending_deletions.iter().enumerate() {
        let remote = manifest_entries.get(iso).cloned();

        if remote.as_ref().is_none_or(|entry| entry.deleted) {
            continue;
        }

        let base_revision = remote.as_ref().map(|entry| entry.revision).unwrap_or(0);

        match sync::delete_entry(iso, base_revision).await {
            Ok(write) => {
                manifest_entries.insert(
                    iso.clone(),
                    ManifestEntry {
                        date: iso.clone(),
                        hash: String::new(),
                        size: 0,
                        revision: write.revision,
                        deleted: true,
                    },
                );
                local.remove(iso);
                deleted += 1;
            }
            Err(sync::SyncError::Conflict) => {
                if sync::get_entry(iso)
                    .await
                    .map_err(|error| error.message())?
                    .is_some()
                {
                    let mut preserved = HashMap::new();
                    download_entry(&key, iso, &mut preserved, &mut manifest_entries).await?;
                    if let Some(text) = preserved.remove(iso) {
                        local.insert(iso.clone(), text);
                    }
                }

                set_sync_message(
                    &state,
                    "An entry changed on another device while you were deleting it. The newer entry was kept.",
                );
            }
            Err(error) => {
                remaining_deletions.extend_from_slice(&pending_deletions[index..]);
                save_sync_deletions(&remaining_deletions);
                return Err(error.message());
            }
        }
    }

    save_sync_deletions(&remaining_deletions);

    let mut dates: Vec<String> = local.keys().cloned().collect();
    dates.sort();

    for iso in dates {
        let text = local.get(&iso).cloned().unwrap_or_default();
        let local_hash = content_hash(&text);
        let remote_entry = manifest_entries.get(&iso).cloned();

        let action = match &remote_entry {
            None => "upload",
            Some(remote_entry) if remote_entry.hash == local_hash => "keep",
            Some(remote_entry) => {
                let base_hash = base_entries.get(&iso).map(|entry| entry.hash.as_str());

                if base_hash == Some(local_hash.as_str()) {
                    "download"
                } else if base_hash == Some(remote_entry.hash.as_str()) {
                    "upload"
                } else {
                    "conflict"
                }
            }
        };

        match action {
            "upload" => {
                let base_revision = remote_entry.map(|entry| entry.revision).unwrap_or(0);
                let encrypted = encrypt_entry(&key, &iso, &text)
                    .map_err(|error| error.to_string())?;

                match sync::put_entry(&iso, base_revision, encrypted, &local_hash).await {
                    Ok(write) => {
                        manifest_entries.insert(
                            iso.clone(),
                            ManifestEntry {
                                date: iso.clone(),
                                hash: write.hash,
                                size: write.size as u64,
                                revision: write.revision,
                                deleted: false,
                            },
                        );
                        uploaded += 1;
                    }
                    Err(sync::SyncError::Conflict) => {
                        conflicts += 1;
                        handle_remote_conflict(&state, &key, &iso, &text, &mut manifest_entries)
                            .await?;
                    }
                    Err(error) => return Err(error.message()),
                }
            }
            "download" => {
                download_entry(&key, &iso, &mut local, &mut manifest_entries).await?;
                downloaded += 1;
            }
            "conflict" => {
                let conflict = opfs::write_conflict(&iso, &text)
                    .await
                    .map_err(js_message)?;
                let _ = conflict;
                conflicts += 1;
                download_entry(&key, &iso, &mut local, &mut manifest_entries).await?;
                downloaded += 1;
            }
            _ => {}
        }
    }

    let remote_only: Vec<String> = manifest_entries
        .iter()
        .filter(|(iso, entry)| !entry.deleted && !local.contains_key(*iso))
        .map(|(iso, _)| iso.clone())
        .collect();

    for iso in remote_only {
        download_entry(&key, &iso, &mut local, &mut manifest_entries).await?;
        downloaded += 1;
    }

    let mut entries: Vec<ManifestEntry> = manifest_entries.into_values().collect();
    entries.sort_by(|left, right| left.date.cmp(&right.date));
    let mut updated = SyncManifest {
        version: 1,
        updated_at: (js_sys::Date::now() / 1000.0) as i64,
        entries,
    };
    let mut base_revision = remote_revision;
    let mut saved = false;

    for _ in 0..3 {
        let encrypted =
            encrypt_manifest(&key, &updated).map_err(|error| error.to_string())?;
        let hash = bytes_hash(&encrypted);

        match sync::put_manifest(base_revision, encrypted, &hash).await {
            Ok(_) => {
                saved = true;
                break;
            }
            Err(sync::SyncError::Conflict) => {
                let Some(latest) = sync::fetch_manifest().await.map_err(|error| error.message())?
                else {
                    return Err("The remote manifest disappeared during sync.".to_string());
                };
                base_revision = latest.revision;
                let latest = decrypt_manifest(&key, &latest.bytes)
                    .map_err(|error| error.to_string())?;
                updated = merge_manifests(&latest, &updated);
            }
            Err(error) => return Err(error.message()),
        }
    }

    if !saved {
        return Err("The remote journal kept changing. Try again.".to_string());
    }

    state.borrow_mut().sync_base = Some(updated.clone());
    save_sync_base(&updated);
    refresh(state.clone()).await.map_err(js_message)?;
    if reload_editor {
        let selected = state.borrow().selected_iso.clone();
        load_entry(state.clone(), &selected).await.map_err(js_message)?;
    }

    let mut message = format!("Synced {uploaded} up, {downloaded} down.");
    if deleted > 0 {
        message.push_str(&format!(" {deleted} deleted."));
    }
    if conflicts > 0 {
        message.push_str(&format!(" {conflicts} conflict copies saved."));
    }

    Ok(message)
}

fn merge_manifests(latest: &SyncManifest, ours: &SyncManifest) -> SyncManifest {
    let mut entries: HashMap<String, ManifestEntry> = latest
        .entries
        .iter()
        .cloned()
        .map(|entry| (entry.date.clone(), entry))
        .collect();

    for entry in &ours.entries {
        let replace = entries
            .get(&entry.date)
            .map(|remote| entry.revision >= remote.revision)
            .unwrap_or(true);

        if replace {
            entries.insert(entry.date.clone(), entry.clone());
        }
    }

    let mut entries: Vec<ManifestEntry> = entries.into_values().collect();
    entries.sort_by(|left, right| left.date.cmp(&right.date));

    SyncManifest {
        version: 1,
        updated_at: (js_sys::Date::now() / 1000.0) as i64,
        entries,
    }
}

async fn download_entry(
    key: &[u8; 32],
    iso: &str,
    local: &mut HashMap<String, String>,
    manifest_entries: &mut HashMap<String, ManifestEntry>,
) -> Result<(), String> {
    let Some(remote) = sync::get_entry(iso).await.map_err(|error| error.message())? else {
        return Ok(());
    };
    let text = decrypt_entry(key, iso, &remote.bytes).map_err(|error| error.to_string())?;
    opfs::write_entry(iso, &text).await.map_err(js_message)?;
    local.insert(iso.to_string(), text);

    if let Some(entry) = manifest_entries.get_mut(iso) {
        entry.revision = remote.revision;
        entry.hash = remote.hash;
        entry.deleted = false;
    }

    Ok(())
}

async fn handle_remote_conflict(
    state: &SharedState,
    key: &[u8; 32],
    iso: &str,
    local_text: &str,
    manifest_entries: &mut HashMap<String, ManifestEntry>,
) -> Result<(), String> {
    opfs::write_conflict(iso, local_text)
        .await
        .map_err(js_message)?;
    let mut local = HashMap::new();
    download_entry(key, iso, &mut local, manifest_entries).await?;
    let _ = state;
    Ok(())
}

fn render_sync_panel(state: SharedState) {
    let html = {
        let state = state.borrow();
        sync_panel_html(&state)
    };

    state.borrow().sync_content.set_inner_html(&html);
}

fn sync_panel_html(state: &State) -> String {
        let mut html = String::new();

        if let Some(message) = &state.sync_message {
            html.push_str(&format!(
                r#"<p class="mb-4 rounded border border-outline-variant/40 bg-surface-container px-3 py-2 font-ui text-ui-label text-on-surface-variant">{}</p>"#,
                escape_sync(message)
            ));
        }

        if state.sync_signed_out {
            html.push_str(
                r#"<p class="font-ui text-ui-body text-on-surface-variant">Sign in to enable encrypted sync. Local journaling stays free.</p>
<div class="mt-4 flex gap-2">
  <a href="/signin" class="rounded bg-primary px-4 py-2 font-ui text-ui-label text-on-primary hover:bg-primary-container hover:text-on-primary-container">Sign in</a>
  <a href="/signup" class="rounded border border-outline-variant/60 px-4 py-2 font-ui text-ui-label text-on-surface hover:bg-surface-container">Create an account</a>
</div>"#,
            );
            return html;
        }

        let Some(status) = &state.sync_status else {
            html.push_str(r#"<p class="font-ui text-ui-label text-on-surface-variant/60">Loading sync status…</p>"#);
            return html;
        };

        if state.sync_busy {
            html.push_str(r#"<p class="font-ui text-ui-body text-on-surface-variant">Working…</p>"#);
            return html;
        }

        if !status.storage_ready {
            html.push_str(
                r#"<p class="font-ui text-ui-body text-on-surface-variant">Encrypted sync storage is not configured on this server. Local journaling is unaffected.</p>"#,
            );
            return html;
        }

        if !status.active {
            html.push_str(
                r#"<p class="font-ui text-ui-body text-on-surface-variant">Sync needs an active plan. Your local journal is unaffected.</p>
<a href="/billing" class="mt-4 inline-flex rounded bg-primary px-4 py-2 font-ui text-ui-label text-on-primary hover:bg-primary-container hover:text-on-primary-container">Choose a plan</a>"#,
            );
            return html;
        }

        if !status.linked {
            html.push_str(
                r#"<p class="font-ui text-ui-body text-on-surface-variant">Create a sync passphrase. It encrypts your journal on this device and is never sent to the server.</p>
<div class="mt-4 space-y-3">
  <label class="block"><span class="font-ui text-ui-label text-on-surface-variant">Sync passphrase</span><input id="sync-passphrase" type="password" minlength="12" autocomplete="new-password" class="mt-1 w-full rounded border border-outline-variant/60 bg-surface px-3 py-2 font-ui text-ui-body text-on-surface outline-none focus:border-primary"></label>
  <label class="block"><span class="font-ui text-ui-label text-on-surface-variant">Confirm passphrase</span><input id="sync-confirm" type="password" minlength="12" autocomplete="new-password" class="mt-1 w-full rounded border border-outline-variant/60 bg-surface px-3 py-2 font-ui text-ui-body text-on-surface outline-none focus:border-primary"></label>
  <button type="button" data-sync-action="setup" class="rounded bg-primary px-4 py-2 font-ui text-ui-label text-on-primary hover:bg-primary-container hover:text-on-primary-container">Create sync key</button>
</div>"#,
            );
            return html;
        }

        if state.sync_master_key.is_none() {
            html.push_str(
                r#"<p class="font-ui text-ui-body text-on-surface-variant">Enter your sync passphrase to unlock encrypted sync on this device.</p>
<div class="mt-4 space-y-3">
  <label class="block"><span class="font-ui text-ui-label text-on-surface-variant">Sync passphrase</span><input id="sync-passphrase" type="password" autocomplete="current-password" class="mt-1 w-full rounded border border-outline-variant/60 bg-surface px-3 py-2 font-ui text-ui-body text-on-surface outline-none focus:border-primary"></label>
  <button type="button" data-sync-action="unlock" class="rounded bg-primary px-4 py-2 font-ui text-ui-label text-on-primary hover:bg-primary-container hover:text-on-primary-container">Unlock sync</button>
</div>
<details class="mt-4"><summary class="cursor-pointer font-ui text-ui-label text-on-surface-variant">Use recovery key</summary>
  <div class="mt-3 space-y-3">
    <input id="sync-recovery-input" type="text" autocomplete="off" placeholder="Recovery key" class="w-full rounded border border-outline-variant/60 bg-surface px-3 py-2 font-mono text-ui-label text-on-surface outline-none focus:border-primary">
    <button type="button" data-sync-action="unlock-recovery" class="rounded border border-outline-variant/60 px-4 py-2 font-ui text-ui-label text-on-surface hover:bg-surface-container">Unlock with recovery key</button>
  </div>
</details>"#,
            );
            return html;
        }

        html.push_str(&format!(
            r#"<p class="font-ui text-ui-body text-on-surface">Encrypted sync is on.</p>
<p class="mt-1 font-ui text-ui-label text-on-surface-variant">{} entries · {}</p>"#,
            status.entry_count,
            format_bytes(status.usage_bytes)
        ));

        if let Some(recovery) = &state.sync_recovery {
            html.push_str(&format!(
                r#"<div class="mt-4 rounded border border-secondary/40 bg-secondary/5 p-3">
<p class="font-ui text-ui-label text-on-surface">Save this recovery key now. It will not be shown again.</p>
<code class="mt-2 block break-all rounded bg-surface px-2 py-2 font-mono text-[10px] text-on-surface">{}</code>
</div>"#,
                escape_sync(recovery)
            ));
        }

        html.push_str(
            r#"<div class="mt-5 flex flex-wrap gap-2">
<button type="button" data-sync-action="sync-now" class="rounded bg-primary px-4 py-2 font-ui text-ui-label text-on-primary hover:bg-primary-container hover:text-on-primary-container">Sync now</button>
<button type="button" data-sync-action="lock" class="rounded border border-outline-variant/60 px-4 py-2 font-ui text-ui-label text-on-surface hover:bg-surface-container">Lock</button>
</div>"#,
        );

        html
}

fn set_sync_message(state: &SharedState, message: &str) {
    state.borrow_mut().sync_message = Some(message.to_string());
}

fn input_value(document: &Document, selector: &str) -> String {
    document
        .query_selector(selector)
        .ok()
        .flatten()
        .and_then(|element| element.dyn_into::<HtmlInputElement>().ok())
        .map(|input| input.value())
        .unwrap_or_default()
}

fn load_sync_base() -> Option<SyncManifest> {
    local_storage()?
        .get_item("dainiki-sync-base")
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
}

fn load_sync_deletions() -> Vec<String> {
    local_storage()
        .and_then(|storage| storage.get_item("dainiki-sync-deletions").ok().flatten())
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

fn save_sync_deletions(deletions: &[String]) {
    if let Some(storage) = local_storage() {
        if let Ok(json) = serde_json::to_string(deletions) {
            storage.set_item("dainiki-sync-deletions", &json).ok();
        }
    }
}

fn queue_sync_delete(iso: &str) {
    let mut deletions = load_sync_deletions();
    if !deletions.iter().any(|date| date == iso) {
        deletions.push(iso.to_string());
        save_sync_deletions(&deletions);
    }
}

fn save_sync_base(manifest: &SyncManifest) {
    if let Some(storage) = local_storage() {
        if let Ok(json) = serde_json::to_string(manifest) {
            storage.set_item("dainiki-sync-base", &json).ok();
        }
    }
}

fn format_bytes(bytes: i64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;
    let bytes = bytes.max(0) as f64;

    if bytes >= GB {
        format!("{:.2} GB of 1 GB", bytes / GB)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes / KB)
    } else {
        format!("{} B", bytes as i64)
    }
}

fn js_message(error: JsValue) -> String {
    error
        .as_string()
        .unwrap_or_else(|| "Local storage error.".to_string())
}

fn escape_sync(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn schedule_sync(state: SharedState) {
    if state.borrow().sync_master_key.is_none() {
        return;
    }

    let next_state = state.clone();
    let mut state = state.borrow_mut();
    state.sync_auto_timeout = Some(Timeout::new(1500, move || {
        spawn_result(run_sync_mode(next_state.clone(), false));
    }));
}

fn set_data_status(state: &SharedState, text: &str) {
    state.borrow().data_status.set_text_content(Some(text));
}

async fn delete_current_entry(state: SharedState) -> Result<(), JsValue> {
    let iso = state.borrow().selected_iso.clone();
    let Some(window) = web_sys::window() else {
        return Ok(());
    };

    let confirmed = window
        .confirm_with_message(&format!(
            "Delete the entry for {iso}? This cannot be undone. If sync is on, the deletion syncs to your other devices."
        ))
        .unwrap_or(false);

    if !confirmed {
        return Ok(());
    }

    finish_inline_edit(state.clone(), true).await?;
    save_entry(state.clone()).await?;
    opfs::delete_entry(&iso).await?;
    queue_sync_delete(&iso);

    {
        let mut state = state.borrow_mut();
        state.has_file = false;
        state.base_text.clear();
    }

    load_entry(state.clone(), &iso).await?;
    refresh(state.clone()).await?;
    set_data_status(&state, "Entry deleted.");
    schedule_sync(state.clone());
    close_data(state);
    Ok(())
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
            schedule_sync(state.clone());

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
                schedule_sync(state.clone());
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
