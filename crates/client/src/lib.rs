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
    month_short, month_year, parse_date, parse_entry,
    sync_crypto::{
        ManifestEntry, SyncManifest, bytes_hash, content_hash, create_sync_keys, decrypt_entry,
        decrypt_manifest, encrypt_entry, encrypt_manifest, sync_entry_id,
        unlock_with_passphrase, unlock_with_recovery_key,
    },
    weekday_name,
};
use gloo_timers::callback::Timeout;
use js_sys::{Array, Function, Reflect, Uint8Array};
use render::EntryRecord;
use wasm_bindgen::{JsCast, JsValue, prelude::*};
use wasm_bindgen_futures::{JsFuture, spawn_local};
use web_sys::{
    Blob, ClipboardEvent, Document, Element, Event, File, HtmlAnchorElement, HtmlElement,
    HtmlInputElement, InputEvent, KeyboardEvent, Node, Url,
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
    data_entries: Element,
    data_entry_count: Element,
    data_status: Element,
    archive_overlay: Element,
    archive_content: Element,
    archive_year: i32,
    archive_month: u32,
    conflicts: Vec<opfs::ConflictCopy>,
    conflict_overlay: Element,
    conflict_content: Element,
    conflict_indicator: Element,
    conflict_count: Element,
    conflict_status: Element,
    settings_overlay: Element,
    import_file: HtmlInputElement,
    import_folder: HtmlInputElement,
    inline_iso: Option<String>,
    inline_base: String,
    inline_link: Option<Element>,
    inline_container: Option<Element>,
    inline_editor: Option<HtmlElement>,
    inline_status: Option<Element>,
    inline_saving: bool,
    inline_save_timeout: Option<Timeout>,
    inline_undo_stack: Vec<String>,
    inline_redo_stack: Vec<String>,
    inline_last_snapshot: String,
    inline_history_timeout: Option<Timeout>,
    slash_menu: Option<Element>,
    slash_items: Option<Element>,
    slash_open: bool,
    slash_index: usize,
    slash_kind: Option<EditorKind>,
    slash_anchor: Option<Element>,
    slash_query: String,
    sidebar: Element,
    sidebar_backdrop: Element,
    welcome_overlay: Element,
    storage_status: Element,
    day_label: Element,
    weekday_label: Element,
    month_label: Element,

    save_timeout: Option<Timeout>,
    history_timeout: Option<Timeout>,
    sync_overlay: Element,
    sync_content: Element,
    sync_affordance: Element,
    sync_label: Element,
    sync_offer_title: Element,
    sync_detail: Element,
    sync_cta: Element,
    sync_status: Option<sync::SyncStatus>,
    sync_signed_out: bool,
    sync_keys: Option<sync::SyncKeys>,
    sync_master_key: Option<[u8; 32]>,
    sync_recovery: Option<String>,
    sync_base: Option<SyncManifest>,
    sync_message: Option<String>,
    sync_busy: bool,
    sync_auto_timeout: Option<Timeout>,
    sync_missing: usize,
    sync_download_all: bool,
    sync_auto_merge: bool,
}

type SharedState = Rc<RefCell<State>>;

#[derive(Clone, Copy, PartialEq, Eq)]
enum EditorKind {
    Main,
    Inline,
}

#[derive(Clone, Copy)]
enum SlashAction {
    Prefix(&'static str),
    Wrap(&'static str, &'static str),
    Plain,
    Divider,
}

#[derive(Clone, Copy)]
struct SlashCommand {
    id: &'static str,
    label: &'static str,
    icon: &'static str,
    keywords: &'static str,
    action: SlashAction,
}

const SLASH_COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        id: "text",
        label: "Text",
        icon: "¶",
        keywords: "paragraph plain body remove formatting",
        action: SlashAction::Plain,
    },
    SlashCommand {
        id: "h1",
        label: "Heading 1",
        icon: "H1",
        keywords: "h1 heading title large",
        action: SlashAction::Prefix("# "),
    },
    SlashCommand {
        id: "h2",
        label: "Heading 2",
        icon: "H2",
        keywords: "h2 heading subtitle title medium",
        action: SlashAction::Prefix("## "),
    },
    SlashCommand {
        id: "h3",
        label: "Heading 3",
        icon: "H3",
        keywords: "h3 heading subtitle title small",
        action: SlashAction::Prefix("### "),
    },
    SlashCommand {
        id: "bullet",
        label: "Bulleted list",
        icon: "•",
        keywords: "bullet list unordered point",
        action: SlashAction::Prefix("- "),
    },
    SlashCommand {
        id: "numbered",
        label: "Numbered list",
        icon: "1.",
        keywords: "number numbered list ordered",
        action: SlashAction::Prefix("1. "),
    },
    SlashCommand {
        id: "quote",
        label: "Quote",
        icon: "❝",
        keywords: "quote blockquote citation",
        action: SlashAction::Prefix("> "),
    },
    SlashCommand {
        id: "divider",
        label: "Divider",
        icon: "—",
        keywords: "divider rule separator horizontal line",
        action: SlashAction::Divider,
    },
    SlashCommand {
        id: "bold",
        label: "Bold",
        icon: "B",
        keywords: "bold strong emphasis",
        action: SlashAction::Wrap("**", "**"),
    },
    SlashCommand {
        id: "italic",
        label: "Italic",
        icon: "I",
        keywords: "italic emphasis",
        action: SlashAction::Wrap("*", "*"),
    },
    SlashCommand {
        id: "strike",
        label: "Strikethrough",
        icon: "S",
        keywords: "strike strikethrough crossed out",
        action: SlashAction::Wrap("~~", "~~"),
    },
    SlashCommand {
        id: "code",
        label: "Inline code",
        icon: "`",
        keywords: "code inline monospace",
        action: SlashAction::Wrap("`", "`"),
    },
];

fn prepare_editor_shell(document: &Document, editor: &HtmlElement) -> Result<(), JsValue> {
    let placeholder = editor.get_attribute("data-placeholder");

    if placeholder.is_none() {
        editor.set_attribute(
            "data-placeholder",
            "Start writing, or type / for commands…",
        )?;
    }

    // A page cached before the placeholder/slash-menu release can be paired
    // with a newer client. Keep the hint working until that shell refreshes.
    if placeholder.is_none() && document.query_selector("#dainiki-shell-style")?.is_none() {
        let style = document.create_element("style")?;
        style.set_id("dainiki-shell-style");
        style.set_text_content(Some(
            r#".live-editor{position:relative}.live-editor[data-empty="true"]::before{content:attr(data-placeholder);position:absolute;inset-inline-start:0;top:0;pointer-events:none;color:color-mix(in oklab,var(--color-on-surface-variant) 48%,transparent)}"#,
        ));

        if let Some(head) = document.query_selector("head")? {
            head.append_child(style.as_ref())?;
        }
    }

    Ok(())
}

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
    prepare_editor_shell(&document, &editor)?;
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
        data_entries: element(&document, "#data-entries")?,
        data_entry_count: element(&document, "#data-entry-count")?,
        data_status: element(&document, "#data-status")?,
        archive_overlay: element(&document, "#archive-overlay")?,
        archive_content: element(&document, "#archive-content")?,
        archive_year: calendar_year,
        archive_month: calendar_month,
        conflicts: Vec::new(),
        conflict_overlay: element(&document, "#conflict-overlay")?,
        conflict_content: element(&document, "#conflict-content")?,
        conflict_indicator: element(&document, "#open-conflicts")?,
        conflict_count: element(&document, "#conflict-count")?,
        conflict_status: element(&document, "#conflict-status")?,
        settings_overlay: element(&document, "#settings-overlay")?,
        import_file: query::<HtmlInputElement>(&document, "#import-file")?,
        import_folder: query::<HtmlInputElement>(&document, "#import-folder-input")?,
        inline_iso: None,
        inline_base: String::new(),
        inline_link: None,
        inline_container: None,
        inline_editor: None,
        inline_status: None,
        inline_saving: false,
        inline_save_timeout: None,
        inline_undo_stack: Vec::new(),
        inline_redo_stack: Vec::new(),
        inline_last_snapshot: String::new(),
        inline_history_timeout: None,
        slash_menu: document.query_selector("#slash-menu")?,
        slash_items: document.query_selector("#slash-items")?,
        slash_open: false,
        slash_index: 0,
        slash_kind: None,
        slash_anchor: None,
        slash_query: String::new(),
        sidebar: element(&document, "aside")?,
        sidebar_backdrop: element(&document, "#sidebar-backdrop")?,
        welcome_overlay: element(&document, "#welcome-overlay")?,
        storage_status: element(&document, "#storage-status")?,
        day_label: element(&document, "#today-day")?,
        weekday_label: element(&document, "#today-weekday")?,
        month_label: element(&document, "#today-month")?,

        save_timeout: None,
        history_timeout: None,
        sync_overlay: element(&document, "#sync-overlay")?,
        sync_content: element(&document, "#sync-content")?,
        sync_affordance: element(&document, "#open-sync")?,
        sync_label: element(&document, "#sync-label")?,
        sync_offer_title: element(&document, "#sync-offer-title")?,
        sync_detail: element(&document, "#sync-detail")?,
        sync_cta: element(&document, "#sync-cta")?,
        sync_status: None,
        sync_signed_out: false,
        sync_keys: None,
        sync_master_key: None,
        sync_recovery: None,
        sync_base: None,
        sync_message: None,
        sync_busy: false,
        sync_auto_timeout: None,
        sync_missing: 0,
        sync_download_all: false,
        sync_auto_merge: local_storage()
            .and_then(|storage| storage.get_item("dainiki-sync-auto-merge").ok().flatten())
            .as_deref()
            == Some("true"),
    }));

    init_theme(&document)?;
    apply_zen(&document, zen_enabled())?;
    init_editor_preferences(&state, &document)?;
    bind_events(&state, &document)?;
    update_labels(&state);

    spawn_result(register_service_worker());
    spawn_result(init(state.clone()));

    Ok(())
}

async fn register_service_worker() -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let container = window.navigator().service_worker();

    JsFuture::from(container.register("/sw.js?v=4")).await?;
    Ok(())
}

fn theme_mode() -> String {
    if let Some(storage) = local_storage() {
        if let Ok(Some(mode)) = storage.get_item("dainiki-theme-mode") {
            if matches!(mode.as_str(), "light" | "dark" | "system") {
                return mode;
            }
        }
        if let Ok(Some(mode)) = storage.get_item("dainiki-theme") {
            if matches!(mode.as_str(), "light" | "dark") {
                return mode;
            }
        }
    }

    "system".to_string()
}

fn resolved_theme(mode: &str) -> &'static str {
    match mode {
        "light" => "light",
        "dark" => "dark",
        _ => {
            let system_dark = web_sys::window()
                .and_then(|window| {
                    window
                        .match_media("(prefers-color-scheme: dark)")
                        .ok()
                        .flatten()
                })
                .is_some_and(|query| query.matches());
            if system_dark { "dark" } else { "light" }
        }
    }
}

fn init_theme(document: &Document) -> Result<(), JsValue> {
    let mode = theme_mode();
    apply_theme(document, resolved_theme(&mode))
}

fn apply_theme(document: &Document, theme: &str) -> Result<(), JsValue> {
    let root = document
        .document_element()
        .ok_or_else(|| JsValue::from_str("no document element"))?;
    root.set_attribute("data-theme", theme)?;

    if let Some(meta) = document.query_selector("meta[name=\"theme-color\"]")? {
        meta.set_attribute(
            "content",
            if theme == "dark" { "#151312" } else { "#fef8f6" },
        )?;
    }

    if let Some(button) = document.query_selector("#theme-toggle")? {
        button.set_attribute("aria-pressed", if theme == "dark" { "true" } else { "false" })?;
    }

    let _ = render_settings(document);
    Ok(())
}

fn set_theme_mode(document: &Document, mode: &str) -> Result<(), JsValue> {
    let next = resolved_theme(mode);
    if let Some(storage) = local_storage() {
        storage.set_item("dainiki-theme-mode", mode)?;
        storage.set_item("dainiki-theme", next)?;
    }
    transition_theme(document, next)
}

fn transition_theme(document: &Document, next: &str) -> Result<(), JsValue> {
    let root = document
        .document_element()
        .ok_or_else(|| JsValue::from_str("no document element"))?;
    let current = root
        .get_attribute("data-theme")
        .unwrap_or_else(|| "light".to_string());
    if current == next {
        return apply_theme(document, next);
    }

    let reduce_motion = web_sys::window()
        .and_then(|window| window.match_media("(prefers-reduced-motion: reduce)").ok().flatten())
        .is_some_and(|query| query.matches());

    if !reduce_motion {
        if let Ok(start) = Reflect::get(
            document.as_ref(),
            &JsValue::from_str("startViewTransition"),
        ) {
            if start.is_function() {
                let callback_document = document.clone();
                let callback_theme = next.to_string();
                let callback = Closure::once_into_js(move || {
                    let _ = apply_theme(&callback_document, &callback_theme);
                });

                let start: &Function = start.unchecked_ref();
                if Reflect::apply(start, document.as_ref(), &Array::of1(&callback)).is_ok()
                {
                    return Ok(());
                }
            }
        }
    }

    apply_theme(document, next)
}

fn toggle_theme(document: &Document) -> Result<(), JsValue> {
    let root = document
        .document_element()
        .ok_or_else(|| JsValue::from_str("no document element"))?;
    let current = root
        .get_attribute("data-theme")
        .unwrap_or_else(|| "light".to_string());
    let next = if current == "dark" { "light" } else { "dark" };
    set_theme_mode(document, next)
}

fn editor_size() -> String {
    local_storage()
        .and_then(|storage| storage.get_item("dainiki-editor-size").ok().flatten())
        .filter(|size| matches!(size.as_str(), "compact" | "default" | "large"))
        .unwrap_or_else(|| "default".to_string())
}

fn apply_editor_size(document: &Document, size: &str) -> Result<(), JsValue> {
    let root = document
        .document_element()
        .ok_or_else(|| JsValue::from_str("no document element"))?;
    let size = if matches!(size, "compact" | "large") {
        size
    } else {
        "default"
    };
    root.set_attribute("data-editor-size", size)
}

fn spellcheck_enabled() -> bool {
    local_storage()
        .and_then(|storage| storage.get_item("dainiki-spellcheck").ok().flatten())
        .as_deref()
        != Some("false")
}

fn apply_spellcheck(state: &SharedState, enabled: bool) {
    state
        .borrow()
        .editor
        .set_attribute("spellcheck", if enabled { "true" } else { "false" })
        .ok();
    if let Some(editor) = state.borrow().inline_editor.clone() {
        editor
            .set_attribute("spellcheck", if enabled { "true" } else { "false" })
            .ok();
    }
}

fn zen_enabled() -> bool {
    local_storage()
        .and_then(|storage| storage.get_item("dainiki-zen").ok().flatten())
        .as_deref()
        == Some("true")
}

fn apply_zen(document: &Document, enabled: bool) -> Result<(), JsValue> {
    let root = document
        .document_element()
        .ok_or_else(|| JsValue::from_str("no document element"))?;

    if enabled {
        root.set_attribute("data-zen", "true")?;
    } else {
        root.remove_attribute("data-zen").ok();
    }

    if let Some(storage) = local_storage() {
        storage
            .set_item("dainiki-zen", if enabled { "true" } else { "false" })
            .ok();
    }

    if let Some(button) = document.query_selector("#toggle-zen")? {
        button.set_attribute(
            "aria-label",
            if enabled { "Exit zen mode" } else { "Enter zen mode" },
        )?;
    }

    Ok(())
}

fn toggle_zen(document: &Document) -> Result<(), JsValue> {
    let root = document
        .document_element()
        .ok_or_else(|| JsValue::from_str("no document element"))?;
    let enabled = root.get_attribute("data-zen").as_deref() != Some("true");
    apply_zen(document, enabled)
}

fn init_editor_preferences(state: &SharedState, document: &Document) -> Result<(), JsValue> {
    apply_editor_size(document, &editor_size())?;
    apply_spellcheck(state, spellcheck_enabled());
    render_settings(document)
}

fn set_spellcheck_from_input(state: SharedState) {
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return;
    };
    let enabled = document
        .query_selector("#setting-spellcheck")
        .ok()
        .flatten()
        .and_then(|input| input.dyn_into::<HtmlInputElement>().ok())
        .is_some_and(|input| input.checked());
    if let Some(storage) = local_storage() {
        storage
            .set_item("dainiki-spellcheck", if enabled { "true" } else { "false" })
            .ok();
    }
    apply_spellcheck(&state, enabled);
}

fn render_settings(document: &Document) -> Result<(), JsValue> {
    let mode = theme_mode();
    let size = editor_size();
    let spellcheck = spellcheck_enabled();

    for value in ["light", "dark", "system"] {
        if let Ok(Some(button)) = document.query_selector(&format!(
            "[data-setting-theme=\"{value}\"]"
        )) {
            button.set_attribute(
                "aria-pressed",
                if value == mode { "true" } else { "false" },
            )?;
        }
    }

    for value in ["compact", "default", "large"] {
        if let Ok(Some(button)) = document.query_selector(&format!(
            "[data-setting-size=\"{value}\"]"
        )) {
            button.set_attribute(
                "aria-pressed",
                if value == size { "true" } else { "false" },
            )?;
        }
    }

    if let Ok(Some(input)) = document.query_selector("#setting-spellcheck") {
        if let Ok(input) = input.dyn_into::<HtmlInputElement>() {
            input.set_checked(spellcheck);
        }
    }

    Ok(())
}

fn open_settings(state: SharedState) {
    if let Some(document) = web_sys::window().and_then(|window| window.document()) {
        let _ = render_settings(&document);
    }
    state
        .borrow()
        .settings_overlay
        .remove_attribute("hidden")
        .ok();

    if let Some(document) = web_sys::window().and_then(|window| window.document()) {
        if let Ok(Some(button)) = document.query_selector("#close-settings") {
            if let Ok(button) = button.dyn_into::<HtmlElement>() {
                button.focus().ok();
            }
        }
    }
}

fn close_settings(state: SharedState) {
    state
        .borrow()
        .settings_overlay
        .set_attribute("hidden", "true")
        .ok();

    if let Some(document) = web_sys::window().and_then(|window| window.document()) {
        if let Ok(Some(button)) = document.query_selector("#open-settings") {
            if let Ok(button) = button.dyn_into::<HtmlElement>() {
                button.focus().ok();
            }
        }
    }
}

fn handle_settings_click(state: SharedState, event: Event) {
    let Some(target) = target_element(&event) else {
        return;
    };

    if let Ok(Some(button)) = target.closest("[data-setting-theme]") {
        if let Some(mode) = button.get_attribute("data-setting-theme") {
            if let Some(document) = web_sys::window().and_then(|window| window.document()) {
                let _ = set_theme_mode(&document, &mode);
                let _ = render_settings(&document);
            }
        }
        return;
    }

    if let Ok(Some(button)) = target.closest("[data-setting-size]") {
        if let Some(size) = button.get_attribute("data-setting-size") {
            if let Some(storage) = local_storage() {
                storage.set_item("dainiki-editor-size", &size).ok();
            }
            if let Some(document) = web_sys::window().and_then(|window| window.document()) {
                let _ = apply_editor_size(&document, &size);
                let _ = render_settings(&document);
            }
        }
        return;
    }

    let Ok(Some(action)) = target.closest("[data-settings-action]") else {
        return;
    };
    let Some(action) = action.get_attribute("data-settings-action") else {
        return;
    };

    close_settings(state.clone());
    match action.as_str() {
        "archive" => open_archive(state),
        "sync" => open_sync(state),
        "data" => open_data(state),
        "conflicts" => open_conflicts(state),
        "zen" => {
            if let Some(document) = web_sys::window().and_then(|window| window.document()) {
                let _ = toggle_zen(&document);
            }
        }
        _ => {}
    }
}

async fn init(state: SharedState) -> Result<(), JsValue> {
    let iso = state.borrow().selected_iso.clone();
    load_entry(state.clone(), &iso).await?;
    refresh(state.clone()).await?;
    maybe_show_welcome(&state);
    spawn_result(auto_start_sync(state));
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

fn editor_element(state: &State, kind: EditorKind) -> Option<HtmlElement> {
    match kind {
        EditorKind::Main => Some(state.editor.clone()),
        EditorKind::Inline => state.inline_editor.clone(),
    }
}

fn editor_kind_for_node(state: &State, node: &Node) -> Option<EditorKind> {
    if state
        .inline_editor
        .as_ref()
        .is_some_and(|editor| editor.contains(Some(node)))
    {
        return Some(EditorKind::Inline);
    }

    state
        .editor
        .contains(Some(node))
        .then_some(EditorKind::Main)
}

fn editor_kind_for_selection(state: &State, document: &Document) -> Option<EditorKind> {
    let node = document.get_selection().ok()??.anchor_node()?;
    editor_kind_for_node(state, &node)
}

fn source_for_editor(state: &State, kind: EditorKind) -> String {
    editor_element(state, kind)
        .map(|editor| markdown::source_from_editor(&editor))
        .unwrap_or_default()
}

fn bind_events(state: &SharedState, document: &Document) -> Result<(), JsValue> {
    let editor = state.borrow().editor.clone();

    let input_state = state.clone();
    let input_document = document.clone();
    listen(
        editor.as_ref(),
        "input",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            if input_state.borrow().composing {
                return;
            }

            refresh_editor_chrome(&input_state, &input_document, EditorKind::Main);
            schedule_save(input_state.clone());
            schedule_history(input_state.clone(), EditorKind::Main);
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
    let composition_document = document.clone();
    listen(
        editor.as_ref(),
        "compositionend",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            composition_end.borrow_mut().composing = false;
            refresh_editor_chrome(&composition_end, &composition_document, EditorKind::Main);
            schedule_save(composition_end.clone());
            schedule_history(composition_end.clone(), EditorKind::Main);
        }),
    )?;

    let paste_state = state.clone();
    let paste_document = document.clone();
    listen(
        editor.as_ref(),
        "paste",
        Closure::<dyn FnMut(Event)>::new(move |event| {
            handle_paste(paste_state.clone(), &paste_document, event, EditorKind::Main);
        }),
    )?;

    let blur_state = state.clone();
    listen(
        editor.as_ref(),
        "blur",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            close_slash_menu(&blur_state);
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

    let beforeinput_state = state.clone();
    let beforeinput_document = document.clone();
    listen(
        editor.as_ref(),
        "beforeinput",
        Closure::<dyn FnMut(Event)>::new(move |event| {
            handle_beforeinput(
                beforeinput_state.clone(),
                &beforeinput_document,
                event,
                EditorKind::Main,
            );
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

    if let Some(slash_menu) = state.borrow().slash_menu.clone() {
        let slash_pointerdown_state = state.clone();
        listen(
            slash_menu.as_ref(),
            "pointerdown",
            Closure::<dyn FnMut(Event)>::new(move |event: Event| {
                if slash_pointerdown_state.borrow().slash_open {
                    event.prevent_default();
                }
            }),
        )?;

        let slash_click_state = state.clone();
        let slash_click_document = document.clone();
        listen(
            slash_menu.as_ref(),
            "click",
            Closure::<dyn FnMut(Event)>::new(move |event| {
                handle_slash_click(
                    slash_click_state.clone(),
                    &slash_click_document,
                    event,
                );
            }),
        )?;

        for event_name in ["wheel", "touchmove"] {
            let dismiss_state = state.clone();
            listen(
                document.as_ref(),
                event_name,
                Closure::<dyn FnMut(Event)>::new(move |event: Event| {
                    let (open, inside) = {
                        let state = dismiss_state.borrow();
                        let inside = target_element(&event).is_some_and(|target| {
                            state
                                .slash_menu
                                .as_ref()
                                .is_some_and(|menu| menu.contains(Some(&target)))
                        });
                        (state.slash_open, inside)
                    };

                    if open && !inside {
                        close_slash_menu(&dismiss_state);
                    }
                }),
            )?;
        }
    }

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

            if target.closest("a[href]").ok().flatten().is_some() {
                return;
            }

            let Ok(Some(link)) = target.closest("[data-entry-card][data-iso]") else {
                return;
            };

            let Some(iso) = link.get_attribute("data-iso") else {
                return;
            };

            event.prevent_default();
            spawn_result(start_inline_edit(timeline_state.clone(), iso));
        }),
    )?;

    let timeline_key_state = state.clone();
    listen(
        state.borrow().timeline.as_ref(),
        "keydown",
        Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let Ok(event) = event.dyn_into::<KeyboardEvent>() else {
                return;
            };

            if event.key() != "Enter" && event.key() != " " {
                return;
            }

            let Some(target) = target_element(&event) else {
                return;
            };
            let Ok(Some(card)) = target.closest("[data-entry-card][data-iso]") else {
                return;
            };
            let Some(iso) = card.get_attribute("data-iso") else {
                return;
            };

            event.prevent_default();
            spawn_result(start_inline_edit(timeline_key_state.clone(), iso));
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

    let open_conflicts_state = state.clone();
    listen(
        element(document, "#open-conflicts")?.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            open_conflicts(open_conflicts_state.clone());
        }),
    )?;

    let close_conflicts_state = state.clone();
    listen(
        element(document, "#close-conflicts")?.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            close_conflicts(close_conflicts_state.clone());
        }),
    )?;

    let conflicts_backdrop_state = state.clone();
    listen(
        state.borrow().conflict_overlay.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |event| {
            let Some(target) = target_element(&event) else {
                return;
            };
            if target.id() == "conflict-overlay" {
                close_conflicts(conflicts_backdrop_state.clone());
            }
        }),
    )?;

    let conflicts_action_state = state.clone();
    listen(
        state.borrow().conflict_content.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |event| {
            handle_conflict_click(conflicts_action_state.clone(), event);
        }),
    )?;

    let conflicts_key_state = state.clone();
    listen(
        state.borrow().conflict_overlay.as_ref(),
        "keydown",
        Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let Ok(event) = event.dyn_into::<KeyboardEvent>() else {
                return;
            };
            if event.key() == "Escape" {
                event.prevent_default();
                close_conflicts(conflicts_key_state.clone());
            }
        }),
    )?;

    let open_archive_state = state.clone();
    listen(
        element(document, "#open-archive")?.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            open_archive(open_archive_state.clone());
        }),
    )?;

    let close_archive_state = state.clone();
    listen(
        element(document, "#close-archive")?.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            close_archive(close_archive_state.clone());
        }),
    )?;

    let archive_backdrop_state = state.clone();
    listen(
        state.borrow().archive_overlay.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |event| {
            let Some(target) = target_element(&event) else {
                return;
            };
            if target.id() == "archive-overlay" {
                close_archive(archive_backdrop_state.clone());
            }
        }),
    )?;

    let archive_action_state = state.clone();
    listen(
        state.borrow().archive_content.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |event| {
            handle_archive_click(archive_action_state.clone(), event);
        }),
    )?;

    let archive_key_state = state.clone();
    listen(
        state.borrow().archive_overlay.as_ref(),
        "keydown",
        Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let Ok(event) = event.dyn_into::<KeyboardEvent>() else {
                return;
            };
            if event.key() == "Escape" {
                event.prevent_default();
                close_archive(archive_key_state.clone());
            }
        }),
    )?;

    let sidebar_search_state = state.clone();
    listen(
        element(document, "#open-search-sidebar")?.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            open_search(sidebar_search_state.clone());
        }),
    )?;

    let toggle_zen_document = document.clone();
    listen(
        element(document, "#toggle-zen")?.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            let _ = toggle_zen(&toggle_zen_document);
        }),
    )?;

    let exit_zen_document = document.clone();
    listen(
        element(document, "#exit-zen")?.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            let _ = apply_zen(&exit_zen_document, false);
        }),
    )?;

    let open_settings_state = state.clone();
    listen(
        element(document, "#open-settings")?.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            open_settings(open_settings_state.clone());
        }),
    )?;

    let close_settings_state = state.clone();
    listen(
        element(document, "#close-settings")?.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            close_settings(close_settings_state.clone());
        }),
    )?;

    let settings_backdrop_state = state.clone();
    listen(
        state.borrow().settings_overlay.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |event| {
            let Some(target) = target_element(&event) else {
                return;
            };
            if target.id() == "settings-overlay" {
                close_settings(settings_backdrop_state.clone());
            }
        }),
    )?;

    let settings_action_state = state.clone();
    listen(
        state.borrow().settings_overlay.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |event| {
            handle_settings_click(settings_action_state.clone(), event);
        }),
    )?;

    let settings_key_state = state.clone();
    listen(
        state.borrow().settings_overlay.as_ref(),
        "keydown",
        Closure::<dyn FnMut(Event)>::new(move |event: Event| {
            let Ok(event) = event.dyn_into::<KeyboardEvent>() else {
                return;
            };
            if event.key() == "Escape" {
                event.prevent_default();
                close_settings(settings_key_state.clone());
            }
        }),
    )?;

    let spellcheck_state = state.clone();
    listen(
        element(document, "#setting-spellcheck")?.as_ref(),
        "change",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            set_spellcheck_from_input(spellcheck_state.clone());
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

    let data_entries_state = state.clone();
    listen(
        state.borrow().data_entries.as_ref(),
        "click",
        Closure::<dyn FnMut(Event)>::new(move |event| {
            let Some(target) = target_element(&event) else {
                return;
            };
            let Some(control) = target.closest("[data-data-action]").ok().flatten() else {
                return;
            };
            let Some(action) = control.get_attribute("data-data-action") else {
                return;
            };
            let Some(iso) = control.get_attribute("data-iso") else {
                return;
            };

            match action.as_str() {
                "open" => {
                    close_data(data_entries_state.clone());
                    spawn_result(select_date(data_entries_state.clone(), iso));
                }
                "delete" => {
                    spawn_result(delete_entry_at(data_entries_state.clone(), iso));
                }
                _ => {}
            }
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

    if handle_slash_keydown(state.clone(), document, &event) {
        return;
    }

    let modifier = event.meta_key() || event.ctrl_key();

    if modifier && event.key().eq_ignore_ascii_case("z") {
        event.prevent_default();

        let kind = editor_kind_for_selection(&state.borrow(), document)
            .unwrap_or(EditorKind::Main);
        if event.shift_key() {
            redo(state, document, kind);
        } else {
            undo(state, document, kind);
        }

        return;
    }

    if modifier && event.key().eq_ignore_ascii_case("y") {
        event.prevent_default();
        let kind = editor_kind_for_selection(&state.borrow(), document)
            .unwrap_or(EditorKind::Main);
        redo(state, document, kind);
        return;
    }

    let Some(kind) = editor_kind_for_selection(&state.borrow(), document) else {
        return;
    };

    if kind == EditorKind::Inline
        && (event.key() == "Escape" || (modifier && event.key() == "Enter"))
    {
        event.prevent_default();
        spawn_result(finish_inline_edit(state, true));
        return;
    }

    if event.key() == "Enter" {
        if split_line(state, document, kind) {
            event.prevent_default();
        }
        return;
    }

    if event.key() == "Backspace" && merge_line(state, document, kind) {
        event.prevent_default();
    }
}

fn handle_beforeinput(
    state: SharedState,
    document: &Document,
    event: Event,
    kind: EditorKind,
) {
    let Ok(event) = event.dyn_into::<InputEvent>() else {
        return;
    };

    if event.is_composing() {
        return;
    }

    if !matches!(
        event.input_type().as_str(),
        "insertParagraph" | "insertLineBreak"
    ) {
        return;
    }

    if state.borrow().slash_open && apply_selected_slash_command(state.clone(), document) {
        event.prevent_default();
        return;
    }

    close_slash_menu(&state);

    if split_line(state, document, kind) {
        event.prevent_default();
    }
}

fn refresh_editor_chrome(state: &SharedState, document: &Document, kind: EditorKind) {
    let Some(editor) = editor_element(&state.borrow(), kind) else {
        return;
    };

    markdown::update_empty_state(&editor);
    sync_slash_menu(state.clone(), document, kind);
}

fn sync_slash_menu(state: SharedState, document: &Document, kind: EditorKind) {
    let has_menu = {
        let state = state.borrow();
        state.slash_menu.is_some() && state.slash_items.is_some()
    };

    if !has_menu {
        return;
    }

    let Some(editor) = editor_element(&state.borrow(), kind) else {
        close_slash_menu(&state);
        return;
    };
    let Some(line) = caret_line(document, &editor) else {
        close_slash_menu(&state);
        return;
    };
    let source = line.text_content().unwrap_or_default();
    let Some((query, _)) = split_slash_source(&source) else {
        close_slash_menu(&state);
        return;
    };

    let query = query.to_string();
    let reset_selection = {
        let state = state.borrow();
        !state.slash_open
            || state.slash_kind != Some(kind)
            || state.slash_query != query
            || state.slash_anchor.as_ref().is_none_or(|anchor| {
                !anchor.is_same_node(Some(line.as_ref()))
            })
    };

    {
        let mut state = state.borrow_mut();
        state.slash_open = true;
        state.slash_kind = Some(kind);
        state.slash_anchor = Some(line);
        state.slash_query = query;
        if reset_selection {
            state.slash_index = 0;
        }
    }

    render_slash_menu(state);
}

fn render_slash_menu(state: SharedState) {
    let (html, line, selected, menu, items) = {
        let state = state.borrow();
        let commands = filtered_slash_commands(&state.slash_query);
        let selected = state.slash_index.min(commands.len().saturating_sub(1));
        (
            slash_items_html(&commands, selected),
            state.slash_anchor.clone(),
            selected,
            state.slash_menu.clone(),
            state.slash_items.clone(),
        )
    };
    let (Some(line), Some(menu), Some(items)) = (line, menu, items) else {
        return;
    };

    state.borrow_mut().slash_index = selected;

    menu.remove_attribute("hidden").ok();
    items.set_inner_html(&html);
    position_slash_menu(&menu, &line);
}

fn position_slash_menu(menu: &Element, line: &Element) {
    let Some(window) = web_sys::window() else {
        return;
    };

    let rect = line.get_bounding_client_rect();
    let menu_rect = menu.get_bounding_client_rect();
    let viewport_width = window
        .inner_width()
        .ok()
        .and_then(|value| value.as_f64())
        .unwrap_or(1024.0);
    let viewport_height = window
        .inner_height()
        .ok()
        .and_then(|value| value.as_f64())
        .unwrap_or(768.0);
    let margin = 12.0;
    let left = rect
        .x()
        .clamp(margin, (viewport_width - menu_rect.width() - margin).max(margin));
    let mut top = rect.y() + rect.height() + 6.0;

    if top + menu_rect.height() > viewport_height - margin {
        top = (rect.y() - menu_rect.height() - 6.0).max(margin);
    }

    menu.set_attribute(
        "style",
        &format!("left:{left}px;top:{top}px;"),
    )
    .ok();
}

fn close_slash_menu(state: &SharedState) {
    let menu = {
        let mut state = state.borrow_mut();
        let menu = state.slash_menu.clone();

        state.slash_open = false;
        state.slash_index = 0;
        state.slash_kind = None;
        state.slash_anchor = None;
        state.slash_query.clear();

        menu
    };

    if let Some(menu) = menu {
        menu.set_attribute("hidden", "true").ok();
    }
}

fn move_slash_selection(state: SharedState, direction: isize) {
    let length = {
        let state = state.borrow();
        filtered_slash_commands(&state.slash_query).len()
    };

    if length == 0 {
        return;
    }

    {
        let mut state = state.borrow_mut();
        let current = state.slash_index.min(length - 1) as isize;
        state.slash_index = (current + direction).rem_euclid(length as isize) as usize;
    }

    highlight_slash_selection(&state);
}

fn highlight_slash_selection(state: &SharedState) {
    let (menu, selected) = {
        let state = state.borrow();
        (state.slash_menu.clone(), state.slash_index)
    };
    let Some(menu) = menu else {
        return;
    };

    if let Ok(Some(previous)) = menu.query_selector("[data-selected=\"true\"]") {
        previous.remove_attribute("data-selected").ok();
    }

    if let Ok(Some(next)) = menu.query_selector(&format!("[data-slash-index=\"{selected}\"]")) {
        next.set_attribute("data-selected", "true").ok();
        next.scroll_into_view_with_bool(false);
    }
}

fn handle_slash_keydown(
    state: SharedState,
    document: &Document,
    event: &KeyboardEvent,
) -> bool {
    if !state.borrow().slash_open {
        return false;
    }

    match event.key().as_str() {
        "ArrowDown" => {
            event.prevent_default();
            move_slash_selection(state, 1);
            true
        }
        "ArrowUp" => {
            event.prevent_default();
            move_slash_selection(state, -1);
            true
        }
        "Enter" | "Tab"
            if !event.meta_key() && !event.ctrl_key() && !event.alt_key() =>
        {
            if apply_selected_slash_command(state.clone(), document) {
                event.prevent_default();
                true
            } else {
                close_slash_menu(&state);
                false
            }
        }
        "Escape" => {
            event.prevent_default();
            close_slash_menu(&state);
            true
        }
        _ => false,
    }
}

fn handle_slash_click(state: SharedState, document: &Document, event: Event) {
    let Some(target) = target_element(&event) else {
        return;
    };
    let Ok(Some(button)) = target.closest("[data-slash-command]") else {
        return;
    };
    let Some(id) = button.get_attribute("data-slash-command") else {
        return;
    };

    event.prevent_default();

    if !apply_slash_command(state.clone(), document, &id) {
        close_slash_menu(&state);
    }
}

fn apply_selected_slash_command(state: SharedState, document: &Document) -> bool {
    let Some(id) = ({
        let state = state.borrow();
        let commands = filtered_slash_commands(&state.slash_query);
        commands
            .get(state.slash_index.min(commands.len().saturating_sub(1)))
            .map(|command| command.id)
    }) else {
        return false;
    };

    apply_slash_command(state, document, id)
}

fn apply_slash_command(state: SharedState, document: &Document, id: &str) -> bool {
    let Some(kind) = state.borrow().slash_kind else {
        return false;
    };
    let Some(editor) = editor_element(&state.borrow(), kind) else {
        return false;
    };
    let Some(line) = caret_line(document, &editor) else {
        close_slash_menu(&state);
        return false;
    };
    let source = line.text_content().unwrap_or_default();
    let Some((_, remainder)) = split_slash_source(&source) else {
        close_slash_menu(&state);
        return false;
    };
    let Some(command) = SLASH_COMMANDS.iter().find(|command| command.id == id) else {
        return false;
    };

    close_slash_menu(&state);
    push_current_history(&state, kind);

    let result = slash_result(command, remainder);

    if matches!(command.action, SlashAction::Divider) {
        markdown::render_line_into(&line, &result, false);

        if let Some(next) = insert_line_after(document, &line, remainder, true) {
            place_caret(document, &next, true);
        } else {
            place_caret(document, &line, false);
        }
    } else {
        markdown::render_line_into(&line, &result, true);

        if matches!(command.action, SlashAction::Prefix(_)) && remainder.is_empty() {
            // Keep the prefix as plain text while the line is being edited so
            // typed text lands after it, not inside the marker span.
            line.set_text_content(Some(&result));
            place_caret(document, &line, false);
        } else {
            let offset = match command.action {
                SlashAction::Wrap(open, _) if remainder.is_empty() => utf16_len(open),
                _ => utf16_len(&result),
            };

            place_caret_at_offset(document, &line, offset);
        }
    }

    editor.focus().ok();
    markdown::update_empty_state(&editor);
    sync_snapshot(&state, kind);
    schedule_save_kind(state, kind);
    true
}

fn slash_items_html(commands: &[&'static SlashCommand], selected: usize) -> String {
    if commands.is_empty() {
        return r#"<div class="slash-empty">No matching blocks</div>"#.to_string();
    }

    let mut html = String::new();

    for (index, command) in commands.iter().enumerate() {
        let selected = if index == selected {
            r#" data-selected="true""#
        } else {
            ""
        };

        html.push_str(&format!(
            r#"<button type="button" class="slash-item" data-slash-index="{index}" data-slash-command="{id}"{selected}><span class="slash-icon" aria-hidden="true">{icon}</span><span class="slash-label">{label}</span></button>"#,
            index = index,
            id = command.id,
            selected = selected,
            icon = command.icon,
            label = command.label,
        ));
    }

    html
}

fn split_slash_source(source: &str) -> Option<(&str, &str)> {
    let rest = source.strip_prefix('/')?;

    if rest.chars().next().is_some_and(char::is_whitespace) {
        return None;
    }

    let trimmed = rest.trim();

    if !trimmed.is_empty()
        && SLASH_COMMANDS.iter().any(|command| {
            command.id.eq_ignore_ascii_case(trimmed)
                || command.label.eq_ignore_ascii_case(trimmed)
        })
    {
        return Some((trimmed, ""));
    }

    let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
    let query = &rest[..end];

    if !query
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_'))
    {
        return None;
    }

    Some((query, rest[end..].trim_start()))
}

fn slash_result(command: &SlashCommand, remainder: &str) -> String {
    match command.action {
        SlashAction::Prefix(prefix) => format!("{prefix}{remainder}"),
        SlashAction::Wrap(open, close) => format!("{open}{remainder}{close}"),
        SlashAction::Plain => remainder.to_string(),
        SlashAction::Divider => "---".to_string(),
    }
}

fn filtered_slash_commands(query: &str) -> Vec<&'static SlashCommand> {
    let query = query.to_lowercase();
    let query = query.as_str();

    SLASH_COMMANDS
        .iter()
        .filter(|command| {
            query.is_empty()
                || command.id.contains(query)
                || command.label.to_lowercase().contains(query)
                || command.keywords.contains(query)
        })
        .collect()
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
        return;
    }

    if modifier && event.key() == "," {
        event.prevent_default();
        open_settings(state);
        return;
    }

    if modifier && event.key() == "." {
        event.prevent_default();
        if let Some(document) = web_sys::window().and_then(|window| window.document()) {
            let _ = toggle_zen(&document);
        }
        return;
    }

    if event.key() == "Escape" && zen_enabled() && state.borrow().inline_iso.is_none() {
        let in_dialog = web_sys::window()
            .and_then(|window| window.document())
            .and_then(|document| document.active_element())
            .and_then(|active| active.closest("[role=\"dialog\"]").ok().flatten())
            .is_some();

        if !in_dialog {
            event.prevent_default();
            if let Some(document) = web_sys::window().and_then(|window| window.document()) {
                let _ = apply_zen(&document, false);
            }
        }
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
    render_data_entries(state.clone());
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

fn open_archive(state: SharedState) {
    let selected = state.borrow().selected_iso.clone();
    let (year, month) = month_parts(&selected);
    {
        let mut state = state.borrow_mut();
        state.archive_year = year;
        state.archive_month = month;
    }
    render_archive(state.clone());
    state
        .borrow()
        .archive_overlay
        .remove_attribute("hidden")
        .ok();

    if let Ok(Some(button)) = state.borrow().archive_content.query_selector("button") {
        button.dyn_into::<HtmlElement>().ok().and_then(|button| {
            button.focus().ok();
            Some(())
        });
    }
}

fn close_archive(state: SharedState) {
    state
        .borrow()
        .archive_overlay
        .set_attribute("hidden", "true")
        .ok();

    if let Some(document) = web_sys::window().and_then(|window| window.document()) {
        if let Ok(Some(button)) = document.query_selector("#open-archive") {
            if let Ok(button) = button.dyn_into::<HtmlElement>() {
                button.focus().ok();
            }
        }
    }
}

fn render_archive(state: SharedState) {
    let (year, month, selected, today, entries) = {
        let state = state.borrow();
        (
            state.archive_year,
            state.archive_month,
            state.selected_iso.clone(),
            state.today_iso.clone(),
            state.entries.clone(),
        )
    };

    let html = render::archive(year, month, &selected, &today, &entries);
    state.borrow().archive_content.set_inner_html(&html);
}

async fn refresh_conflicts_only(state: SharedState) -> Result<(), JsValue> {
    let conflicts = opfs::list_conflicts().await?;
    state.borrow_mut().conflicts = conflicts;
    render_conflict_indicator(state.clone());
    render_conflict_panel(state);
    Ok(())
}

fn render_conflict_indicator(state: SharedState) {
    let count = state.borrow().conflicts.len();
    let (indicator, count_label) = {
        let state = state.borrow();
        (state.conflict_indicator.clone(), state.conflict_count.clone())
    };

    if count == 0 {
        indicator.set_attribute("hidden", "true").ok();
    } else {
        indicator.remove_attribute("hidden").ok();
    }
    count_label.set_text_content(Some(&count.to_string()));
}

fn render_conflict_panel(state: SharedState) {
    let html = {
        let state = state.borrow();
        render::conflict_panel(&state.conflicts, &state.entries)
    };
    state.borrow().conflict_content.set_inner_html(&html);
}

fn open_conflicts(state: SharedState) {
    set_conflict_status(&state, "");
    render_conflict_panel(state.clone());
    state
        .borrow()
        .conflict_overlay
        .remove_attribute("hidden")
        .ok();

    if let Some(document) = web_sys::window().and_then(|window| window.document()) {
        if let Ok(Some(button)) = document.query_selector("#close-conflicts") {
            if let Ok(button) = button.dyn_into::<HtmlElement>() {
                button.focus().ok();
            }
        }
    }
}

fn close_conflicts(state: SharedState) {
    state
        .borrow()
        .conflict_overlay
        .set_attribute("hidden", "true")
        .ok();

    let indicator_visible = !state
        .borrow()
        .conflict_indicator
        .has_attribute("hidden");
    let selector = if indicator_visible {
        "#open-conflicts"
    } else {
        "#open-sync"
    };

    if let Some(document) = web_sys::window().and_then(|window| window.document()) {
        if let Ok(Some(button)) = document.query_selector(selector) {
            if let Ok(button) = button.dyn_into::<HtmlElement>() {
                button.focus().ok();
            }
        }
    }
}

fn set_conflict_status(state: &SharedState, text: &str) {
    state
        .borrow()
        .conflict_status
        .set_text_content(Some(text));
}

fn handle_conflict_click(state: SharedState, event: Event) {
    let Some(target) = target_element(&event) else {
        return;
    };
    let Ok(Some(button)) = target.closest("[data-conflict-action]") else {
        return;
    };
    let Some(action) = button.get_attribute("data-conflict-action") else {
        return;
    };
    let Some(name) = button.get_attribute("data-conflict-name") else {
        return;
    };
    let Some(iso) = button.get_attribute("data-conflict-iso") else {
        return;
    };

    let Some(conflict) = state
        .borrow()
        .conflicts
        .iter()
        .find(|conflict| conflict.name == name && conflict.iso == iso)
        .cloned()
    else {
        set_conflict_status(&state, "That conflict copy is no longer available.");
        return;
    };

    let Some(window) = web_sys::window() else {
        return;
    };

    match action.as_str() {
        "use" => {
            let confirmed = window
                .confirm_with_message(
                    "Replace the current entry with this conflict copy? The conflict copy will be removed.",
                )
                .unwrap_or(false);
            if !confirmed {
                return;
            }

            let next_state = state.clone();
            spawn_result(async move {
                opfs::write_entry(&iso, &conflict.text).await?;
                opfs::delete_conflict(&iso, &name).await?;
                refresh(next_state.clone()).await?;
                if next_state.borrow().selected_iso == iso {
                    load_entry(next_state.clone(), &iso).await?;
                }
                schedule_sync(next_state.clone());
                set_conflict_status(&next_state, "Current entry replaced. Conflict copy removed.");
                Ok(())
            });
        }
        "download" => {
            if let Some(document) = conflict_content_document(&state) {
                if let Err(error) = download_bytes(&document, conflict.text.as_bytes(), &name) {
                    web_sys::console::error_1(&error);
                    set_conflict_status(&state, "Could not download the conflict copy.");
                }
            }
        }
        "delete" => {
            let confirmed = window
                .confirm_with_message("Delete this conflict copy? This cannot be undone.")
                .unwrap_or(false);
            if !confirmed {
                return;
            }

            let next_state = state.clone();
            spawn_result(async move {
                opfs::delete_conflict(&iso, &name).await?;
                refresh(next_state.clone()).await?;
                set_conflict_status(&next_state, "Conflict copy deleted.");
                Ok(())
            });
        }
        "open" => {
            close_conflicts(state.clone());
            spawn_result(select_date(state, iso));
        }
        _ => {}
    }
}

fn conflict_content_document(state: &SharedState) -> Option<Document> {
    state.borrow().conflict_content.owner_document()
}

fn handle_archive_click(state: SharedState, event: Event) {
    let Some(target) = target_element(&event) else {
        return;
    };

    if let Ok(Some(year)) = target.closest("[data-archive-year]") {
        if let Some(value) = year.get_attribute("data-archive-year") {
            if let Ok(value) = value.parse::<i32>() {
                state.borrow_mut().archive_year = value;
                render_archive(state);
            }
        }
        return;
    }

    if let Ok(Some(month)) = target.closest("[data-archive-month]") {
        if let Some(value) = month.get_attribute("data-archive-month") {
            if let Ok(value) = value.parse::<u32>() {
                if (1..=12).contains(&value) {
                    state.borrow_mut().archive_month = value;
                    render_archive(state);
                }
            }
        }
        return;
    }

    if let Ok(Some(day)) = target.closest("[data-archive-day]") {
        if let Some(iso) = day.get_attribute("data-archive-day") {
            close_archive(state.clone());
            spawn_result(select_date(state, iso));
        }
        return;
    }

    if target.closest("[data-archive-today]").ok().flatten().is_some() {
        let today = state.borrow().today_iso.clone();
        close_archive(state.clone());
        spawn_result(select_date(state, today));
        return;
    }

    if target
        .closest("[data-archive-search]")
        .ok()
        .flatten()
        .is_some()
    {
        close_archive(state.clone());
        open_search(state);
    }
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

            let user_id = state.borrow().sync_status.as_ref().map(|status| status.user_id);
            if linked && state.borrow().sync_master_key.is_none() {
                if let Some(key) = user_id.and_then(remembered_sync_key) {
                    let mut state = state.borrow_mut();
                    state.sync_master_key = Some(key);
                    state.sync_base = load_sync_base();
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

    render_sync_panel(state.clone());
    update_sync_affordance(state);
    Ok(())
}

async fn auto_start_sync(state: SharedState) -> Result<(), JsValue> {
    match sync::fetch_status().await {
        Ok(status) => {
            let user_id = status.user_id;
            let should_sync = status.active && status.linked && status.storage_ready;
            {
                let mut state = state.borrow_mut();
                state.sync_status = Some(status);
                state.sync_signed_out = false;
            }

            if should_sync {
                if let Some(key) = remembered_sync_key(user_id) {
                    {
                        let mut state = state.borrow_mut();
                        state.sync_master_key = Some(key);
                        state.sync_base = load_sync_base();
                    }
                    update_sync_affordance(state.clone());
                    run_sync_mode(state.clone(), true).await?;
                }
            }
        }
        Err(sync::SyncError::Unauthorized) => {
            let mut state = state.borrow_mut();
            state.sync_status = None;
            state.sync_signed_out = true;
        }
        Err(error) => {
            state.borrow_mut().sync_message = Some(error.message());
        }
    }

    update_sync_affordance(state);
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
        "auto-merge" => {
            let enabled = target
                .closest("input")
                .ok()
                .flatten()
                .and_then(|input| input.dyn_into::<HtmlInputElement>().ok())
                .is_some_and(|input| input.checked());
            state.borrow_mut().sync_auto_merge = enabled;
            if let Some(storage) = local_storage() {
                storage
                    .set_item(
                        "dainiki-sync-auto-merge",
                        if enabled { "true" } else { "false" },
                    )
                    .ok();
            }
            render_sync_panel(state);
        }
        "sync-all" => {
            state.borrow_mut().sync_download_all = true;
            spawn_result(run_sync(state));
        }
        "forget" => forget_sync_device(state),
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
        state.sync_missing = 0;
        state.sync_download_all = false;
        if let Some(status) = &mut state.sync_status {
            status.linked = true;
        }
    }

    remember_sync_key(&state);
    set_sync_message(
        &state,
        "Sync is on. Save the recovery key below; this device will stay connected automatically.",
    );
    render_sync_panel(state.clone());
    update_sync_affordance(state.clone());
    run_sync(state).await
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
        state.sync_missing = 0;
        state.sync_download_all = false;
    }

    remember_sync_key(&state);
    render_sync_panel(state.clone());
    update_sync_affordance(state.clone());
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
        state.sync_missing = 0;
        state.sync_download_all = false;
    }

    remember_sync_key(&state);
    render_sync_panel(state.clone());
    update_sync_affordance(state.clone());
    run_sync(state).await
}

fn forget_sync_device(state: SharedState) {
    let confirmed = web_sys::window()
        .and_then(|window| {
            window
                .confirm_with_message(
                    "Forget the sync key on this device? Sync will ask for your passphrase next time. Your journal stays here.",
                )
                .ok()
        })
        .unwrap_or(false);

    if !confirmed {
        return;
    }

    clear_remembered_sync_key();
    {
        let mut state = state.borrow_mut();
        state.sync_master_key = None;
        state.sync_recovery = None;
        state.sync_missing = 0;
        state.sync_download_all = false;
        state.sync_message = Some(
            "Sync key forgotten on this device. Enter your passphrase to reconnect.".to_string(),
        );
    }
    render_sync_panel(state.clone());
    update_sync_affordance(state);
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
    update_sync_affordance(state.clone());

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
    render_sync_panel(state.clone());
    update_sync_affordance(state);
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

    for entry in &mut manifest.entries {
        if entry.sync_id.is_empty() {
            entry.sync_id = sync_entry_id(&key, &entry.date);
        }
    }
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
    let mut auto_merged = 0usize;
    let auto_merge = state.borrow().sync_auto_merge;

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

        opfs::delete_base(&tombstone.date)
            .await
            .map_err(js_message)?;
    }

    let pending_deletions = load_sync_deletions();
    let mut remaining_deletions = Vec::new();

    for (index, iso) in pending_deletions.iter().enumerate() {
        let remote = manifest_entries.get(iso).cloned();

        if remote.as_ref().is_none_or(|entry| entry.deleted) {
            continue;
        }

        let base_revision = remote.as_ref().map(|entry| entry.revision).unwrap_or(0);
        let sync_id = remote
            .as_ref()
            .map(|entry| entry.sync_id.clone())
            .unwrap_or_else(|| sync_entry_id(&key, iso));

        match sync::delete_entry(&sync_id, base_revision).await {
            Ok(write) => {
                manifest_entries.insert(
                    iso.clone(),
                    ManifestEntry {
                        date: iso.clone(),
                        sync_id,
                        hash: String::new(),
                        size: 0,
                        revision: write.revision,
                        deleted: true,
                    },
                );
                local.remove(iso);
                opfs::delete_base(iso).await.map_err(js_message)?;
                deleted += 1;
            }
            Err(sync::SyncError::Conflict) => {
                if sync::get_entry(&sync_id)
                    .await
                    .map_err(|error| error.message())?
                    .is_some()
                {
                    let mut preserved = HashMap::new();
                    download_entry(&key, iso, &sync_id, &mut preserved, &mut manifest_entries)
                        .await?;
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
                let base_revision = remote_entry.as_ref().map(|entry| entry.revision).unwrap_or(0);
                let sync_id = remote_entry
                    .as_ref()
                    .map(|entry| entry.sync_id.clone())
                    .unwrap_or_else(|| sync_entry_id(&key, &iso));
                let encrypted = encrypt_entry(&key, &iso, &text)
                    .map_err(|error| error.to_string())?;

                match sync::put_entry(&sync_id, base_revision, encrypted, &local_hash).await {
                    Ok(write) => {
                        manifest_entries.insert(
                            iso.clone(),
                            ManifestEntry {
                                date: iso.clone(),
                                sync_id,
                                hash: write.hash,
                                size: write.size as u64,
                                revision: write.revision,
                                deleted: false,
                            },
                        );
                        opfs::write_base(&iso, &text).await.map_err(js_message)?;
                        uploaded += 1;
                    }
                    Err(sync::SyncError::Conflict) => {
                        conflicts += 1;
                        handle_remote_conflict(
                            &state,
                            &key,
                            &iso,
                            &sync_id,
                            &text,
                            &mut manifest_entries,
                        )
                        .await?;
                    }
                    Err(error) => return Err(error.message()),
                }
            }
            "keep" => {
                opfs::write_base(&iso, &text).await.map_err(js_message)?;
            }
            "download" => {
                let sync_id = remote_entry
                    .as_ref()
                    .map(|entry| entry.sync_id.clone())
                    .unwrap_or_else(|| sync_entry_id(&key, &iso));
                download_entry(&key, &iso, &sync_id, &mut local, &mut manifest_entries).await?;
                downloaded += 1;
            }
            "conflict" => {
                let sync_id = remote_entry
                    .as_ref()
                    .map(|entry| entry.sync_id.clone())
                    .unwrap_or_else(|| sync_entry_id(&key, &iso));

                if auto_merge {
                    if let Some(entry) = remote_entry.as_ref() {
                        if let Some(base_text) = opfs::read_base(&iso).await.map_err(js_message)? {
                            if let Some(remote) = sync::get_entry(&sync_id)
                                .await
                                .map_err(|error| error.message())?
                            {
                                let remote_text = decrypt_entry(&key, &iso, &remote.bytes)
                                    .map_err(|error| error.to_string())?;

                                if let Ok(merged) = diffy::merge(&base_text, &text, &remote_text) {
                                    let merged_hash = content_hash(&merged);
                                    let encrypted = encrypt_entry(&key, &iso, &merged)
                                        .map_err(|error| error.to_string())?;

                                    match sync::put_entry(
                                        &sync_id,
                                        entry.revision,
                                        encrypted,
                                        &merged_hash,
                                    )
                                    .await
                                    {
                                        Ok(write) => {
                                            opfs::write_entry(&iso, &merged)
                                                .await
                                                .map_err(js_message)?;
                                            opfs::write_base(&iso, &merged)
                                                .await
                                                .map_err(js_message)?;
                                            local.insert(iso.clone(), merged);
                                            manifest_entries.insert(
                                                iso.clone(),
                                                ManifestEntry {
                                                    date: iso.clone(),
                                                    sync_id,
                                                    hash: write.hash,
                                                    size: write.size as u64,
                                                    revision: write.revision,
                                                    deleted: false,
                                                },
                                            );
                                            uploaded += 1;
                                            auto_merged += 1;
                                            continue;
                                        }
                                        Err(sync::SyncError::Conflict) => {}
                                        Err(error) => return Err(error.message()),
                                    }
                                }
                            }
                        }
                    }
                }

                opfs::write_conflict(&iso, &text)
                    .await
                    .map_err(js_message)?;
                conflicts += 1;
                download_entry(&key, &iso, &sync_id, &mut local, &mut manifest_entries).await?;
                downloaded += 1;
            }
            _ => {}
        }
    }

    let mut remote_only: Vec<String> = manifest_entries
        .iter()
        .filter(|(iso, entry)| !entry.deleted && !local.contains_key(*iso))
        .map(|(iso, _)| iso.clone())
        .collect();
    remote_only.sort_by(|left, right| right.cmp(left));

    let download_all = state.borrow().sync_download_all;
    let limit = if download_all { usize::MAX } else { 30 };

    for iso in remote_only.into_iter().take(limit) {
        let sync_id = manifest_entries
            .get(&iso)
            .map(|entry| entry.sync_id.clone())
            .unwrap_or_else(|| sync_entry_id(&key, &iso));
        download_entry(&key, &iso, &sync_id, &mut local, &mut manifest_entries).await?;
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
                let mut latest = decrypt_manifest(&key, &latest.bytes)
                    .map_err(|error| error.to_string())?;
                for entry in &mut latest.entries {
                    if entry.sync_id.is_empty() {
                        entry.sync_id = sync_entry_id(&key, &entry.date);
                    }
                }
                updated = merge_manifests(&latest, &updated);
            }
            Err(error) => return Err(error.message()),
        }
    }

    if !saved {
        return Err("The remote journal kept changing. Try again.".to_string());
    }

    let missing = updated
        .entries
        .iter()
        .filter(|entry| !entry.deleted && !local.contains_key(&entry.date))
        .count();

    {
        let mut state = state.borrow_mut();
        state.sync_base = Some(updated.clone());
        state.sync_missing = missing;
        if missing == 0 {
            state.sync_download_all = false;
        }
    }
    save_sync_base(&updated);
    refresh(state.clone()).await.map_err(js_message)?;
    if reload_editor {
        let selected = state.borrow().selected_iso.clone();
        load_entry(state.clone(), &selected).await.map_err(js_message)?;
    }

    let mut message = format!("Synced {uploaded} up, {downloaded} down.");
    if auto_merged > 0 {
        message.push_str(&format!(" {auto_merged} auto-merged."));
    }
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
    sync_id: &str,
    local: &mut HashMap<String, String>,
    manifest_entries: &mut HashMap<String, ManifestEntry>,
) -> Result<(), String> {
    let Some(remote) = sync::get_entry(sync_id).await.map_err(|error| error.message())? else {
        return Ok(());
    };
    let text = decrypt_entry(key, iso, &remote.bytes).map_err(|error| error.to_string())?;
    opfs::write_entry(iso, &text).await.map_err(js_message)?;
    opfs::write_base(iso, &text).await.map_err(js_message)?;
    local.insert(iso.to_string(), text);

    if let Some(entry) = manifest_entries.get_mut(iso) {
        entry.revision = remote.revision;
        entry.hash = remote.hash;
        entry.sync_id = sync_id.to_string();
        entry.deleted = false;
    }

    Ok(())
}

async fn handle_remote_conflict(
    state: &SharedState,
    key: &[u8; 32],
    iso: &str,
    sync_id: &str,
    local_text: &str,
    manifest_entries: &mut HashMap<String, ManifestEntry>,
) -> Result<(), String> {
    opfs::write_conflict(iso, local_text)
        .await
        .map_err(js_message)?;
    let mut local = HashMap::new();
    download_entry(key, iso, sync_id, &mut local, manifest_entries).await?;
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

        let plan_label = match status.plan.as_deref() {
            Some("yearly") => "Yearly plan",
            Some("monthly") => "Monthly plan",
            _ => "Sync account",
        };
        html.push_str(&format!(
            r#"<p class="mb-3 truncate font-ui text-ui-label text-on-surface-variant/70">{}</p>"#,
            escape_sync(&format!(
                "{} · {}",
                status.email,
                if status.active {
                    plan_label
                } else {
                    "No active plan"
                }
            ))
        ));

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
                r#"<p class="font-ui text-ui-body text-on-surface-variant">Your plan includes encrypted sync. Set a passphrase once; after that this device stays connected automatically. The passphrase is never sent to the server.</p>
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
                r#"<p class="font-ui text-ui-body text-on-surface-variant">Enter your sync passphrase once to connect this device. After that, paid sync stays on automatically.</p>
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

        if state.sync_missing > 0 {
            html.push_str(&format!(
                r#"<div class="mt-3 rounded border border-outline-variant/40 bg-surface-container/60 px-3 py-2">
<p class="font-ui text-ui-label text-on-surface-variant">{} older entries are not downloaded on this device yet.</p>
<button type="button" data-sync-action="sync-all" class="mt-2 rounded border border-outline-variant/60 px-3 py-1.5 font-ui text-ui-label text-on-surface hover:bg-surface-container">Download all entries</button>
</div>"#,
                state.sync_missing
            ));
        }

        if let Some(recovery) = &state.sync_recovery {
            html.push_str(&format!(
                r#"<div class="mt-4 rounded border border-secondary/40 bg-secondary/5 p-3">
<p class="font-ui text-ui-label text-on-surface">Save this recovery key now. It will not be shown again.</p>
<code class="mt-2 block break-all rounded bg-surface px-2 py-2 font-mono text-[10px] text-on-surface">{}</code>
</div>"#,
                escape_sync(recovery)
            ));
        }

        let auto_merge_checked = if state.sync_auto_merge { " checked" } else { "" };
        html.push_str(&format!(
            r#"<label class="mt-4 flex items-start gap-2 rounded border border-outline-variant/40 bg-surface-container-low/50 px-3 py-2 font-ui text-ui-label text-on-surface-variant">
<input id="sync-auto-merge" type="checkbox" data-sync-action="auto-merge" class="mt-0.5 accent-primary"{auto_merge_checked}>
<span><span class="block text-on-surface">Automatically merge clean conflicts</span><span class="mt-0.5 block text-[10px] leading-4 text-on-surface-variant/60">Non-overlapping edits are combined. Overlapping edits still create a conflict copy.</span></span>
</label>"#
        ));

        html.push_str(
            r#"<div class="mt-5 flex flex-wrap gap-2">
<button type="button" data-sync-action="sync-now" class="rounded bg-primary px-4 py-2 font-ui text-ui-label text-on-primary hover:bg-primary-container hover:text-on-primary-container">Sync now</button>
<button type="button" data-sync-action="forget" class="rounded border border-outline-variant/60 px-4 py-2 font-ui text-ui-label text-on-surface-variant hover:bg-surface-container hover:text-on-surface">Forget this device</button>
</div>
<p class="mt-3 font-ui text-[10px] text-on-surface-variant/60">Sync stays on automatically while this account is active. Forget the device to require your passphrase again.</p>"#,
        );

        html
}

fn set_sync_message(state: &SharedState, message: &str) {
    state.borrow_mut().sync_message = Some(message.to_string());
}

const SYNC_DEVICE_KEY: &str = "dainiki-sync-device";

fn remember_sync_key(state: &SharedState) {
    let (Some(key), Some(user_id)) = (
        state.borrow().sync_master_key,
        state
            .borrow()
            .sync_status
            .as_ref()
            .map(|status| status.user_id),
    ) else {
        return;
    };

    if let Some(storage) = local_storage() {
        storage
            .set_item(SYNC_DEVICE_KEY, &format!("{user_id}:{}", key_hex(&key)))
            .ok();
    }
}

fn remembered_sync_key(user_id: i64) -> Option<[u8; 32]> {
    let storage = local_storage()?;
    let stored = storage.get_item(SYNC_DEVICE_KEY).ok().flatten()?;
    let (stored_user, key) = stored.split_once(':')?;

    if stored_user.parse::<i64>().ok() != Some(user_id) {
        storage.remove_item(SYNC_DEVICE_KEY).ok();
        return None;
    }

    key_from_hex(key)
}

fn clear_remembered_sync_key() {
    if let Some(storage) = local_storage() {
        storage.remove_item(SYNC_DEVICE_KEY).ok();
    }
}

fn key_hex(key: &[u8; 32]) -> String {
    let mut output = String::with_capacity(64);
    for byte in key {
        use std::fmt::Write as _;
        let _ = write!(&mut output, "{byte:02x}");
    }
    output
}

fn key_from_hex(value: &str) -> Option<[u8; 32]> {
    if value.len() != 64 {
        return None;
    }

    let mut key = [0_u8; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        let text = std::str::from_utf8(chunk).ok()?;
        key[index] = u8::from_str_radix(text, 16).ok()?;
    }
    Some(key)
}

fn update_sync_affordance(state: SharedState) {
    let (label, title, detail, cta, state_name) = {
        let state = state.borrow();

        if state.sync_signed_out {
            (
                "Dainiki Sync",
                "Keep your journal on every device.",
                "End-to-end encrypted. $2/month or $20/year.".to_string(),
                "Sign in or sign up",
                "off",
            )
        } else if let Some(status) = &state.sync_status {
            if state.sync_busy {
                (
                    "Syncing",
                    "Bringing your journal up to date.",
                    "Encrypted in transit and at rest.".to_string(),
                    "View sync status",
                    "busy",
                )
            } else if !status.active {
                (
                    "Signed in",
                    "Put your words on every screen.",
                    format!("No active plan · {}", status.email),
                    "Choose a plan",
                    "setup",
                )
            } else if !status.storage_ready {
                (
                    "Paid sync",
                    "Your local journal is ready.",
                    "Sync unavailable".to_string(),
                    "View status",
                    "off",
                )
            } else if !status.linked {
                (
                    "Paid sync",
                    "One last step.",
                    format!("Finish setup · {}", status.email),
                    "Finish setup",
                    "setup",
                )
            } else if state.sync_master_key.is_none() {
                (
                    "Paid sync",
                    "Connect this device.",
                    "Connect this device with your passphrase.".to_string(),
                    "Connect device",
                    "setup",
                )
            } else if state.sync_missing > 0 {
                (
                    "Paid sync",
                    "Your journal is in sync.",
                    format!(
                        "On · {} · {} older",
                        status.email, state.sync_missing
                    ),
                    "Download older entries",
                    "on",
                )
            } else {
                (
                    "Paid sync",
                    "Every device in step.",
                    format!("On · {}", status.email),
                    "Manage sync",
                    "on",
                )
            }
        } else {
            (
                "Journal",
                "Checking this device…",
                "Checking sync…".to_string(),
                "View sync",
                "off",
            )
        }
    };

    let state = state.borrow();
    state.sync_label.set_text_content(Some(label));
    state
        .sync_label
        .set_attribute("data-sync-state", state_name)
        .ok();
    state.sync_offer_title.set_text_content(Some(title));
    state.sync_detail.set_text_content(Some(&detail));
    state.sync_cta.set_text_content(Some(cta));
    state
        .sync_affordance
        .set_attribute("data-sync-state", state_name)
        .ok();
    state
        .sync_affordance
        .set_attribute("aria-label", &format!("{title} {cta}"))
        .ok();
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

fn entry_label(iso: &str) -> String {
    parse_date(iso).map_or_else(
        || iso.to_string(),
        |date| {
            format!(
                "{}, {} {}, {}",
                weekday_name(date),
                month_short(date),
                date.day(),
                date.year()
            )
        },
    )
}

async fn delete_entry_at(state: SharedState, iso: String) -> Result<(), JsValue> {
    let label = entry_label(&iso);
    let Some(window) = web_sys::window() else {
        return Ok(());
    };

    let confirmed = window
        .confirm_with_message(&format!(
            "Delete the entry for {label}? This cannot be undone. If sync is on, the deletion syncs to your other devices."
        ))
        .unwrap_or(false);

    if !confirmed {
        return Ok(());
    }

    finish_inline_edit(state.clone(), true).await?;
    save_entry(state.clone()).await?;
    opfs::delete_entry(&iso).await?;
    queue_sync_delete(&iso);

    let current = state.borrow().selected_iso == iso;

    if current {
        {
            let mut state = state.borrow_mut();
            state.has_file = false;
            state.base_text.clear();
        }
        load_entry(state.clone(), &iso).await?;
    }

    refresh(state.clone()).await?;
    set_data_status(&state, &format!("Deleted {label}."));
    schedule_sync(state.clone());
    Ok(())
}

async fn export_zip(state: SharedState) -> Result<(), JsValue> {
    set_data_status(&state, "Preparing export…");

    let entries = opfs::list_entries().await?;
    let conflicts = opfs::list_conflicts().await?;
    let count = entries.len();
    let conflict_count = conflicts.len();
    let options = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));

    for (iso, text) in entries {
        let path = format!("{}/{}/{}.md", &iso[0..4], &iso[5..7], iso);
        writer.start_file(path, options).map_err(zip_error)?;
        writer.write_all(text.as_bytes()).map_err(io_error)?;
    }

    for conflict in conflicts {
        let path = format!(
            "{}/{}/{}",
            &conflict.iso[0..4],
            &conflict.iso[5..7],
            conflict.name
        );
        writer.start_file(path, options).map_err(zip_error)?;
        writer
            .write_all(conflict.text.as_bytes())
            .map_err(io_error)?;
    }

    writer
        .start_file("README.md", options)
        .map_err(zip_error)?;
    writer
        .write_all(
            b"# Dainiki journal\n\nEntries are Markdown files in `YYYY/MM/YYYY-MM-DD.md`.\nConflict copies use the `YYYY-MM-DD.conflict-<timestamp>.md` suffix and are included.\nDainiki is text-only; media files are not included.\n",
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
    set_data_status(
        &state,
        &format!("Exported {count} entries and {conflict_count} conflict copies."),
    );
    Ok(())
}

async fn import_files(state: SharedState, input: HtmlInputElement) -> Result<(), JsValue> {
    let Some(files) = input.files() else {
        set_data_status(&state, "No files selected.");
        return Ok(());
    };

    let mut imported = 0;
    let mut conflicts = 0;
    let mut restored_conflicts = 0;
    let mut skipped = 0;

    for index in 0..files.length() {
        let Some(file) = files.item(index) else {
            continue;
        };

        let name = file.name();
        let lower = name.to_lowercase();

        if lower.ends_with(".zip") {
            let bytes = read_file_bytes(&file).await?;
            let (zip_imported, zip_conflicts, zip_restored, zip_skipped) =
                import_zip(bytes).await?;
            imported += zip_imported;
            conflicts += zip_conflicts;
            restored_conflicts += zip_restored;
            skipped += zip_skipped;
        } else if lower.ends_with(".md") {
            let text = read_file_text(&file).await?;

            if let Some((iso, conflict_stem)) = conflict_from_stem(&name) {
                opfs::write_conflict_named(iso, conflict_stem, &text).await?;
                restored_conflicts += 1;
                continue;
            }

            let Some(iso) = date_from_stem(&name) else {
                skipped += 1;
                continue;
            };

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
        &format!(
            "Imported {imported}, conflicts {conflicts}, restored conflicts {restored_conflicts}, skipped {skipped}."
        ),
    );
    Ok(())
}

async fn import_zip(bytes: Vec<u8>) -> Result<(usize, usize, usize, usize), JsValue> {
    let mut archive = ZipArchive::new(Cursor::new(bytes)).map_err(zip_error)?;
    let mut imported = 0;
    let mut conflicts = 0;
    let mut restored_conflicts = 0;
    let mut skipped = 0;

    for index in 0..archive.len() {
        let mut file = archive.by_index(index).map_err(zip_error)?;
        let name = file.name().map_err(zip_error)?.to_string();

        if !name.to_lowercase().ends_with(".md") {
            skipped += 1;
            continue;
        }

        let stem = name.rsplit('/').next().unwrap_or(&name);
        let mut text = String::new();
        file.read_to_string(&mut text).map_err(io_error)?;

        if let Some((iso, conflict_stem)) = conflict_from_stem(stem) {
            opfs::write_conflict_named(iso, conflict_stem, &text).await?;
            restored_conflicts += 1;
            continue;
        }

        let Some(iso) = date_from_stem(stem) else {
            skipped += 1;
            continue;
        };

        match import_markdown(&iso, &text).await? {
            ImportOutcome::Imported => imported += 1,
            ImportOutcome::Conflict => conflicts += 1,
            ImportOutcome::Unchanged => {}
        }
    }

    Ok((imported, conflicts, restored_conflicts, skipped))
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

fn conflict_from_stem(stem: &str) -> Option<(&str, &str)> {
    let name = stem;
    let stem = stem.strip_suffix(".md")?;
    let (iso, suffix) = stem.split_once(".conflict-")?;

    if parse_date(iso).is_none()
        || suffix.is_empty()
        || !suffix.chars().all(|ch| ch.is_ascii_digit())
    {
        return None;
    }

    Some((iso, name))
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

fn record_history(state: &SharedState, kind: EditorKind) {
    let source = source_for_editor(&state.borrow(), kind);
    let mut borrowed = state.borrow_mut();

    match kind {
        EditorKind::Main => {
            if source == borrowed.last_snapshot {
                return;
            }

            let previous = borrowed.last_snapshot.clone();
            borrowed.undo_stack.push(previous);
            if borrowed.undo_stack.len() > 80 {
                borrowed.undo_stack.remove(0);
            }
            borrowed.last_snapshot = source;
            borrowed.redo_stack.clear();
        }
        EditorKind::Inline => {
            if source == borrowed.inline_last_snapshot {
                return;
            }

            let previous = borrowed.inline_last_snapshot.clone();
            borrowed.inline_undo_stack.push(previous);
            if borrowed.inline_undo_stack.len() > 80 {
                borrowed.inline_undo_stack.remove(0);
            }
            borrowed.inline_last_snapshot = source;
            borrowed.inline_redo_stack.clear();
        }
    }
}

fn push_current_history(state: &SharedState, kind: EditorKind) {
    let current = source_for_editor(&state.borrow(), kind);
    let mut borrowed = state.borrow_mut();

    match kind {
        EditorKind::Main => {
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
        EditorKind::Inline => {
            if current == borrowed.inline_last_snapshot {
                return;
            }

            borrowed.inline_undo_stack.push(current.clone());
            if borrowed.inline_undo_stack.len() > 80 {
                borrowed.inline_undo_stack.remove(0);
            }
            borrowed.inline_last_snapshot = current;
            borrowed.inline_redo_stack.clear();
        }
    }
}

fn sync_snapshot(state: &SharedState, kind: EditorKind) {
    let current = source_for_editor(&state.borrow(), kind);
    match kind {
        EditorKind::Main => state.borrow_mut().last_snapshot = current,
        EditorKind::Inline => state.borrow_mut().inline_last_snapshot = current,
    }

    if let Some(editor) = editor_element(&state.borrow(), kind) {
        markdown::update_empty_state(&editor);
    }
}

fn schedule_history(state: SharedState, kind: EditorKind) {
    let next_state = state.clone();
    let mut state = state.borrow_mut();

    match kind {
        EditorKind::Main => {
            state.history_timeout = None;
            state.history_timeout = Some(Timeout::new(350, move || {
                record_history(&next_state, EditorKind::Main);
            }));
        }
        EditorKind::Inline => {
            state.inline_history_timeout = None;
            state.inline_history_timeout = Some(Timeout::new(350, move || {
                record_history(&next_state, EditorKind::Inline);
            }));
        }
    }
}

fn undo(state: SharedState, document: &Document, kind: EditorKind) {
    let current = source_for_editor(&state.borrow(), kind);
    let mut borrowed = state.borrow_mut();

    let previous = match kind {
        EditorKind::Main => borrowed.undo_stack.pop(),
        EditorKind::Inline => borrowed.inline_undo_stack.pop(),
    };
    let Some(previous) = previous else {
        return;
    };

    match kind {
        EditorKind::Main => {
            if current != previous {
                borrowed.redo_stack.push(current);
            }
            borrowed.last_snapshot = previous.clone();
            borrowed.history_timeout = None;
        }
        EditorKind::Inline => {
            if current != previous {
                borrowed.inline_redo_stack.push(current);
            }
            borrowed.inline_last_snapshot = previous.clone();
            borrowed.inline_history_timeout = None;
        }
    }
    drop(borrowed);

    restore_source(&state, document, kind, &previous);
}

fn redo(state: SharedState, document: &Document, kind: EditorKind) {
    let current = source_for_editor(&state.borrow(), kind);
    let mut borrowed = state.borrow_mut();

    let next = match kind {
        EditorKind::Main => borrowed.redo_stack.pop(),
        EditorKind::Inline => borrowed.inline_redo_stack.pop(),
    };
    let Some(next) = next else {
        return;
    };

    match kind {
        EditorKind::Main => {
            borrowed.undo_stack.push(current);
            borrowed.last_snapshot = next.clone();
            borrowed.history_timeout = None;
        }
        EditorKind::Inline => {
            borrowed.inline_undo_stack.push(current);
            borrowed.inline_last_snapshot = next.clone();
            borrowed.inline_history_timeout = None;
        }
    }
    drop(borrowed);

    restore_source(&state, document, kind, &next);
}

fn restore_source(
    state: &SharedState,
    document: &Document,
    kind: EditorKind,
    source: &str,
) {
    close_slash_menu(state);

    let Some(editor) = editor_element(&state.borrow(), kind) else {
        return;
    };
    markdown::render_document(&editor, source);

    if let Ok(Some(line)) = editor.query_selector(".line:last-child") {
        line.class_list().add_1("active").ok();
        place_caret(document, &line, false);
    }

    editor.focus().ok();
    schedule_save_kind(state.clone(), kind);
}

fn handle_paste(
    state: SharedState,
    document: &Document,
    event: Event,
    kind: EditorKind,
) {
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
    close_slash_menu(&state);

    let Some(editor) = editor_element(&state.borrow(), kind) else {
        return;
    };
    let Some(line) = ensure_active_line(document, &editor) else {
        return;
    };

    push_current_history(&state, kind);

    let source = line.text_content().unwrap_or_default();
    let offset = caret_offset(document, &line).unwrap_or_else(|| utf16_len(&source));
    let (before, after) = split_at_utf16(&source, offset);
    let normalized = normalize_newlines(&text);
    let parts: Vec<&str> = normalized.split('\n').collect();

    if parts.len() == 1 {
        let next = format!("{before}{}{after}", parts[0]);
        markdown::render_line_into(&line, &next, true);
        place_caret_at_offset(document, &line, utf16_len(before) + utf16_len(parts[0]));
        sync_snapshot(&state, kind);
        schedule_save_kind(state, kind);
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

    sync_snapshot(&state, kind);
    schedule_save_kind(state, kind);
}

fn ensure_active_line(document: &Document, editor: &Element) -> Option<Element> {
    if let Some(line) = caret_line(document, editor) {
        return Some(line);
    }

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
    let kind = editor_kind_for_selection(&state.borrow(), document);
    let Some(kind) = kind else {
        return;
    };
    let Some(editor) = editor_element(&state.borrow(), kind) else {
        return;
    };
    let new_line = caret_line(document, &editor);
    let old_line = editor.query_selector(".line.active").ok().flatten();

    if old_line == new_line {
        return;
    }

    let (slash_open, slash_same_line) = {
        let state = state.borrow();
        (
            state.slash_open,
            state.slash_anchor.as_ref().is_some_and(|anchor| {
                new_line
                    .as_ref()
                    .is_some_and(|line| anchor.is_same_node(Some(line.as_ref())))
            }),
        )
    };

    if slash_open && !slash_same_line {
        close_slash_menu(&state);
    }

    if let Some(old) = old_line {
        let source = old.text_content().unwrap_or_default();
        markdown::render_line_into(&old, &source, false);
    }

    if let Some(new) = new_line {
        new.class_list().add_1("active").ok();
    }
}

fn split_line(state: SharedState, document: &Document, kind: EditorKind) -> bool {
    close_slash_menu(&state);

    let Some(editor) = editor_element(&state.borrow(), kind) else {
        return false;
    };
    let Some(line) = caret_line(document, &editor) else {
        return false;
    };

    push_current_history(&state, kind);

    let source = line.text_content().unwrap_or_default();
    let offset = caret_offset(document, &line).unwrap_or_else(|| utf16_len(&source));
    let (before, after) = split_at_utf16(&source, offset);

    markdown::render_line_into(&line, before, false);

    let Ok(new_line) = document.create_element("div") else {
        return false;
    };

    markdown::render_line_into(&new_line, after, true);

    let Some(parent) = line.parent_node() else {
        return false;
    };

    let reference = line.next_sibling();
    if parent
        .insert_before(new_line.as_ref(), reference.as_ref())
        .is_err()
    {
        return false;
    }

    place_caret(document, &new_line, true);
    sync_snapshot(&state, kind);
    schedule_save_kind(state, kind);
    true
}

fn merge_line(state: SharedState, document: &Document, kind: EditorKind) -> bool {
    close_slash_menu(&state);

    let Some(editor) = editor_element(&state.borrow(), kind) else {
        return false;
    };
    let Some(line) = caret_line(document, &editor) else {
        return false;
    };

    if caret_offset(document, &line) != Some(0) {
        return false;
    }

    let Some(previous) = line.previous_element_sibling() else {
        return false;
    };

    push_current_history(&state, kind);

    let mut source = previous.text_content().unwrap_or_default();
    source.push_str(&line.text_content().unwrap_or_default());

    markdown::render_line_into(&previous, &source, true);
    line.remove();
    place_caret(document, &previous, false);
    sync_snapshot(&state, kind);
    schedule_save_kind(state, kind);
    true
}

fn active_line(document: &Document) -> Option<Element> {
    let selection = document.get_selection().ok()??;
    let node = selection.anchor_node()?;

    markdown::line_of(&node)
}

fn caret_line(document: &Document, editor: &Element) -> Option<Element> {
    if let Some(line) = active_line(document) {
        return Some(line);
    }

    // Firefox can leave the caret directly in the contenteditable root when an
    // empty line is clicked. Resolve that caret to the nearest line so Enter
    // still splits the block instead of being silently swallowed.
    let selection = document.get_selection().ok()??;
    let anchor = selection.anchor_node()?;

    if !editor.contains(Some(&anchor)) {
        return None;
    }

    let offset = selection.anchor_offset() as usize;
    let mut child = editor.first_child();
    let mut index = 0;
    let mut previous_line = None;

    while let Some(current) = child {
        let line = current
            .dyn_ref::<Element>()
            .filter(|element| element.class_list().contains("line"))
            .cloned();

        if anchor.is_same_node(Some(editor.as_ref())) && index >= offset {
            return line.or(previous_line);
        }

        if !anchor.is_same_node(Some(editor.as_ref()))
            && current.contains(Some(&anchor))
            && let Some(line) = line.as_ref()
        {
            return Some(line.clone());
        }

        if line.is_some() {
            previous_line = line;
        }

        index += 1;
        child = current.next_sibling();
    }

    previous_line
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
    close_slash_menu(&state);

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

            if let Err(error) = refresh_conflicts_only(state.clone()).await {
                web_sys::console::error_1(&error);
            }
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

    {
        let view = state.borrow();
        view.timeline.set_inner_html(&timeline);
        view.tag_list.set_inner_html(&tags);
        view.calendar.set_inner_html(&calendar);

        if filter_visible {
            view.active_filter.remove_attribute("hidden").ok();
        } else {
            view.active_filter.set_attribute("hidden", "true").ok();
        }

        view.active_filter_label
            .set_text_content(Some(&filter_label));
    }

    render_data_entries(state);
}

fn render_data_entries(state: SharedState) {
    let (html, count) = {
        let state = state.borrow();
        (
            render::data_entries(&state.entries, &state.selected_iso),
            state.entries.len(),
        )
    };

    let state = state.borrow();
    state.data_entries.set_inner_html(&html);
    state
        .data_entry_count
        .set_text_content(Some(&format!("{count} {}", if count == 1 { "entry" } else { "entries" })));
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
    let selector = format!("[data-entry-card][data-iso=\"{iso}\"]");
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

    let Some(card) = timeline.query_selector(&format!(
        "[data-entry-card][data-iso=\"{iso}\"]"
    ))? else {
        return Ok(());
    };

    let document = timeline
        .owner_document()
        .ok_or_else(|| JsValue::from_str("no document"))?;
    let container = document.create_element("div")?;
    container.set_inner_html(&render::inline_editor());

    let Some(parent) = card.parent_node() else {
        return Ok(());
    };

    let reference = card.next_sibling();
    parent.insert_before(container.as_ref(), reference.as_ref())?;
    card.set_attribute("hidden", "true")?;

    let editor: HtmlElement = container
        .query_selector(".inline-live")?
        .ok_or_else(|| JsValue::from_str("missing inline editor"))?
        .dyn_into()?;
    let status = container
        .query_selector(".inline-status")?
        .ok_or_else(|| JsValue::from_str("missing inline status"))?;
    markdown::render_document(&editor, &text);
    editor
        .set_attribute(
            "spellcheck",
            if spellcheck_enabled() { "true" } else { "false" },
        )
        .ok();

    let input_state = state.clone();
    let input_document = document.clone();
    listen(
        editor.as_ref(),
        "input",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            if input_state.borrow().composing {
                return;
            }

            refresh_editor_chrome(&input_state, &input_document, EditorKind::Inline);
            schedule_inline_save(input_state.clone());
            schedule_history(input_state.clone(), EditorKind::Inline);
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
    let composition_document = document.clone();
    listen(
        editor.as_ref(),
        "compositionend",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            composition_end.borrow_mut().composing = false;
            refresh_editor_chrome(&composition_end, &composition_document, EditorKind::Inline);
            schedule_inline_save(composition_end.clone());
            schedule_history(composition_end.clone(), EditorKind::Inline);
        }),
    )?;

    let paste_state = state.clone();
    let paste_document = document.clone();
    listen(
        editor.as_ref(),
        "paste",
        Closure::<dyn FnMut(Event)>::new(move |event| {
            handle_paste(
                paste_state.clone(),
                &paste_document,
                event,
                EditorKind::Inline,
            );
        }),
    )?;

    let blur_state = state.clone();
    listen(
        editor.as_ref(),
        "blur",
        Closure::<dyn FnMut(Event)>::new(move |_| {
            close_slash_menu(&blur_state);
            let next_state = blur_state.clone();
            spawn_result(async move { save_inline(next_state).await.map(|_| ()) });
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

    let beforeinput_state = state.clone();
    let beforeinput_document = document.clone();
    listen(
        editor.as_ref(),
        "beforeinput",
        Closure::<dyn FnMut(Event)>::new(move |event| {
            handle_beforeinput(
                beforeinput_state.clone(),
                &beforeinput_document,
                event,
                EditorKind::Inline,
            );
        }),
    )?;

    if let Ok(Some(line)) = editor.query_selector(".line:last-child") {
        line.class_list().add_1("active").ok();
        place_caret(&document, &line, false);
    }

    {
        let mut state = state.borrow_mut();
        state.inline_iso = Some(iso);
        state.inline_base = text.clone();
        state.inline_link = Some(card);
        state.inline_container = Some(container);
        state.inline_editor = Some(editor.clone());
        state.inline_status = Some(status);
        state.inline_saving = false;
        state.inline_save_timeout = None;
        state.inline_undo_stack.clear();
        state.inline_redo_stack.clear();
        state.inline_last_snapshot = text;
        state.inline_history_timeout = None;
    }

    editor.focus().ok();
    Ok(())
}

async fn finish_inline_edit(state: SharedState, save: bool) -> Result<(), JsValue> {
    if state.borrow().inline_iso.is_none() {
        return Ok(());
    }

    close_slash_menu(&state);

    if save {
        state.borrow_mut().inline_save_timeout = None;
        if !save_inline(state.clone()).await? {
            return Ok(());
        }
    }

    let (container, link) = {
        let mut state = state.borrow_mut();
        let container = state.inline_container.take();
        let link = state.inline_link.take();

        state.inline_iso = None;
        state.inline_base.clear();
        state.inline_editor = None;
        state.inline_status = None;
        state.inline_saving = false;
        state.inline_save_timeout = None;
        state.inline_undo_stack.clear();
        state.inline_redo_stack.clear();
        state.inline_last_snapshot.clear();
        state.inline_history_timeout = None;

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
    let (iso, text, base, editor, status) = {
        let state = state.borrow();
        let Some(iso) = state.inline_iso.clone() else {
            return Ok(true);
        };
        let Some(editor) = state.inline_editor.clone() else {
            return Ok(true);
        };

        (
            iso,
            markdown::source_from_editor(&editor),
            state.inline_base.clone(),
            editor,
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
                    markdown::render_document(&editor, &remote);
                    {
                        let mut state = state.borrow_mut();
                        state.inline_base = remote.clone();
                        state.inline_last_snapshot = remote.clone();
                        state.inline_undo_stack.clear();
                        state.inline_redo_stack.clear();
                    }
                    set_inline_status(status.as_ref(), &format!("Conflict copy saved as {name}"));
                    let _ = refresh_conflicts_only(state.clone()).await;
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
            state.borrow_mut().inline_base = text.clone();
            set_inline_status(status.as_ref(), "Saved locally");
            true
        }
        Ok(None) => {
            let conflict = opfs::write_conflict(&iso, &text).await;

            match conflict {
                Ok(name) => {
                    markdown::render_document(&editor, "");
                    {
                        let mut state = state.borrow_mut();
                        state.inline_base.clear();
                        state.inline_last_snapshot.clear();
                        state.inline_undo_stack.clear();
                        state.inline_redo_stack.clear();
                    }
                    set_inline_status(
                        status.as_ref(),
                        &format!("Conflict copy saved as {name} — entry was deleted"),
                    );
                    let _ = refresh_conflicts_only(state.clone()).await;
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
                state.borrow_mut().inline_base = text.clone();
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
}

fn schedule_save_kind(state: SharedState, kind: EditorKind) {
    match kind {
        EditorKind::Main => schedule_save(state),
        EditorKind::Inline => schedule_inline_save(state),
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

#[cfg(test)]
mod tests {
    use super::{
        SLASH_COMMANDS, SlashCommand, filtered_slash_commands, slash_result,
        split_slash_source,
    };

    fn command(id: &str) -> &'static SlashCommand {
        SLASH_COMMANDS
            .iter()
            .find(|command| command.id == id)
            .expect("missing slash command")
    }

    #[test]
    fn slash_source_requires_a_command_token() {
        assert_eq!(split_slash_source("/"), Some(("", "")));
        assert_eq!(split_slash_source("/h1"), Some(("h1", "")));
        assert_eq!(
            split_slash_source("/h1 A title with spaces"),
            Some(("h1", "A title with spaces")),
        );
        assert_eq!(
            split_slash_source("/heading 1"),
            Some(("heading 1", "")),
        );
        assert_eq!(
            split_slash_source("/heading 1 "),
            Some(("heading 1", "")),
        );
        assert_eq!(split_slash_source("plain text"), None);
        assert_eq!(split_slash_source("/ h1"), None);
        assert_eq!(split_slash_source("//example.com"), None);
    }

    #[test]
    fn slash_commands_apply_without_markdown_knowledge() {
        assert_eq!(slash_result(command("h1"), "A title"), "# A title");
        assert_eq!(slash_result(command("bullet"), "Item one"), "- Item one");
        assert_eq!(slash_result(command("bold"), "important"), "**important**");
        assert_eq!(slash_result(command("text"), "plain"), "plain");
        assert_eq!(slash_result(command("divider"), "ignored"), "---");
    }

    #[test]
    fn slash_search_matches_labels_and_keywords() {
        let h1 = filtered_slash_commands("h1");
        assert_eq!(h1.len(), 1);
        assert_eq!(h1[0].id, "h1");

        let headings = filtered_slash_commands("heading");
        assert!(headings.iter().all(|command| command.id.starts_with('h')));
        assert_eq!(headings.len(), 3);

        assert_eq!(filtered_slash_commands("").len(), SLASH_COMMANDS.len());
    }
}
