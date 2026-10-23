use dainiki_core::parse_date;
use js_sys::{AsyncIterator, Reflect};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{
    File, FileSystemDirectoryHandle, FileSystemFileHandle, FileSystemGetDirectoryOptions,
    FileSystemGetFileOptions, FileSystemHandle, FileSystemHandleKind,
    FileSystemWritableFileStream,
};

#[derive(Clone)]
pub struct ConflictCopy {
    pub name: String,
    pub iso: String,
    pub timestamp: u64,
    pub text: String,
}

pub async fn read_entry(iso: &str) -> Result<Option<String>, JsValue> {
    let month_dir = match journal_dir(iso, false).await {
        Ok(dir) => dir,
        Err(error) if is_not_found(&error) => return Ok(None),
        Err(error) => return Err(error),
    };

    let file_handle = match file_handle(&month_dir, &format!("{iso}.md"), false).await {
        Ok(handle) => handle,
        Err(error) if is_not_found(&error) => return Ok(None),
        Err(error) => return Err(error),
    };

    Ok(Some(read_file(&file_handle).await?))
}

pub async fn delete_entry(iso: &str) -> Result<(), JsValue> {
    let month_dir = match journal_dir(iso, false).await {
        Ok(dir) => dir,
        Err(error) if is_not_found(&error) => return Ok(()),
        Err(error) => return Err(error),
    };
    let name = format!("{iso}.md");

    match JsFuture::from(month_dir.remove_entry(&name)).await {
        Ok(_) => Ok(()),
        Err(error) if is_not_found(&error) => Ok(()),
        Err(error) => Err(error),
    }
}

pub async fn write_entry(iso: &str, text: &str) -> Result<(), JsValue> {
    let month_dir = journal_dir(iso, true).await?;
    let file_handle = file_handle(&month_dir, &format!("{iso}.md"), true).await?;

    let writable: FileSystemWritableFileStream = JsFuture::from(file_handle.create_writable())
        .await?
        .dyn_into()?;

    JsFuture::from(writable.write_with_str(text)?).await?;
    JsFuture::from(writable.close()).await?;

    Ok(())
}

pub async fn write_conflict(iso: &str, text: &str) -> Result<String, JsValue> {
    let month_dir = journal_dir(iso, true).await?;
    let stamp = js_sys::Date::now() as u64;
    let name = format!("{iso}.conflict-{stamp}.md");
    let file_handle = file_handle(&month_dir, &name, true).await?;

    let writable: FileSystemWritableFileStream = JsFuture::from(file_handle.create_writable())
        .await?
        .dyn_into()?;

    JsFuture::from(writable.write_with_str(text)?).await?;
    JsFuture::from(writable.close()).await?;

    Ok(name)
}

pub async fn list_entries() -> Result<Vec<(String, String)>, JsValue> {
    let root = storage_root().await?;
    let journal = match directory(&root, "journal", false).await {
        Ok(dir) => dir,
        Err(error) if is_not_found(&error) => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };

    let mut entries = Vec::new();
    let years = journal.values();

    while let Some(handle) = next_handle(&years).await? {
        if handle.kind() != FileSystemHandleKind::Directory {
            continue;
        }

        let year = handle.name();
        if year.len() != 4 || !year.chars().all(|ch| ch.is_ascii_digit()) {
            continue;
        }

        let year_dir: FileSystemDirectoryHandle = handle.unchecked_into();
        let months = year_dir.values();

        while let Some(month) = next_handle(&months).await? {
            if month.kind() != FileSystemHandleKind::Directory {
                continue;
            }

            let month_name = month.name();
            if month_name.len() != 2 || !month_name.chars().all(|ch| ch.is_ascii_digit()) {
                continue;
            }

            let month_dir: FileSystemDirectoryHandle = month.unchecked_into();
            let files = month_dir.values();

            while let Some(file) = next_handle(&files).await? {
                if file.kind() != FileSystemHandleKind::File {
                    continue;
                }

                let name = file.name();
                let Some(stem) = name.strip_suffix(".md") else {
                    continue;
                };

                if stem.len() != 10 || !stem.chars().enumerate().all(|(index, ch)| {
                    if index == 4 || index == 7 {
                        ch == '-'
                    } else {
                        ch.is_ascii_digit()
                    }
                }) {
                    continue;
                }

                let file_handle: FileSystemFileHandle = file.unchecked_into();
                let text = read_file(&file_handle).await?;
                entries.push((stem.to_string(), text));
            }
        }
    }

    Ok(entries)
}

pub async fn list_conflicts() -> Result<Vec<ConflictCopy>, JsValue> {
    let root = storage_root().await?;
    let journal = match directory(&root, "journal", false).await {
        Ok(dir) => dir,
        Err(error) if is_not_found(&error) => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };

    let mut conflicts = Vec::new();
    let years = journal.values();

    while let Some(handle) = next_handle(&years).await? {
        if handle.kind() != FileSystemHandleKind::Directory {
            continue;
        }

        let year = handle.name();
        if year.len() != 4 || !year.chars().all(|ch| ch.is_ascii_digit()) {
            continue;
        }

        let year_dir: FileSystemDirectoryHandle = handle.unchecked_into();
        let months = year_dir.values();

        while let Some(month) = next_handle(&months).await? {
            if month.kind() != FileSystemHandleKind::Directory {
                continue;
            }

            let month_name = month.name();
            if month_name.len() != 2 || !month_name.chars().all(|ch| ch.is_ascii_digit()) {
                continue;
            }

            let month_dir: FileSystemDirectoryHandle = month.unchecked_into();
            let files = month_dir.values();

            while let Some(file) = next_handle(&files).await? {
                if file.kind() != FileSystemHandleKind::File {
                    continue;
                }

                let name = file.name();
                let Some(stem) = name.strip_suffix(".md") else {
                    continue;
                };
                let Some((iso, stamp)) = stem.split_once(".conflict-") else {
                    continue;
                };
                let Some(timestamp) = stamp.parse::<u64>().ok() else {
                    continue;
                };
                if parse_date(iso).is_none() {
                    continue;
                }

                let file_handle: FileSystemFileHandle = file.unchecked_into();
                let text = read_file(&file_handle).await?;
                let iso = iso.to_string();
                conflicts.push(ConflictCopy {
                    name,
                    iso,
                    timestamp,
                    text,
                });
            }
        }
    }

    conflicts.sort_by(|left, right| {
        right
            .iso
            .cmp(&left.iso)
            .then_with(|| right.timestamp.cmp(&left.timestamp))
    });

    Ok(conflicts)
}

pub async fn delete_conflict(iso: &str, name: &str) -> Result<(), JsValue> {
    let month_dir = match journal_dir(iso, false).await {
        Ok(dir) => dir,
        Err(error) if is_not_found(&error) => return Ok(()),
        Err(error) => return Err(error),
    };

    match JsFuture::from(month_dir.remove_entry(name)).await {
        Ok(_) => Ok(()),
        Err(error) if is_not_found(&error) => Ok(()),
        Err(error) => Err(error),
    }
}

pub async fn read_base(iso: &str) -> Result<Option<String>, JsValue> {
    let root = storage_root().await?;
    let base_dir = match directory(&root, "sync-base", false).await {
        Ok(dir) => dir,
        Err(error) if is_not_found(&error) => return Ok(None),
        Err(error) => return Err(error),
    };
    let handle = match file_handle(&base_dir, &format!("{iso}.md"), false).await {
        Ok(handle) => handle,
        Err(error) if is_not_found(&error) => return Ok(None),
        Err(error) => return Err(error),
    };

    Ok(Some(read_file(&handle).await?))
}

pub async fn write_base(iso: &str, text: &str) -> Result<(), JsValue> {
    let root = storage_root().await?;
    let base_dir = directory(&root, "sync-base", true).await?;
    let handle = file_handle(&base_dir, &format!("{iso}.md"), true).await?;
    let writable: FileSystemWritableFileStream = JsFuture::from(handle.create_writable())
        .await?
        .dyn_into()?;

    JsFuture::from(writable.write_with_str(text)?).await?;
    JsFuture::from(writable.close()).await?;
    Ok(())
}

pub async fn delete_base(iso: &str) -> Result<(), JsValue> {
    let root = storage_root().await?;
    let base_dir = match directory(&root, "sync-base", false).await {
        Ok(dir) => dir,
        Err(error) if is_not_found(&error) => return Ok(()),
        Err(error) => return Err(error),
    };

    match JsFuture::from(base_dir.remove_entry(&format!("{iso}.md"))).await {
        Ok(_) => Ok(()),
        Err(error) if is_not_found(&error) => Ok(()),
        Err(error) => Err(error),
    }
}

async fn storage_root() -> Result<FileSystemDirectoryHandle, JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("no window"))?;
    let storage = window.navigator().storage();

    JsFuture::from(storage.get_directory())
        .await?
        .dyn_into()
}

async fn journal_dir(iso: &str, create: bool) -> Result<FileSystemDirectoryHandle, JsValue> {
    let root = storage_root().await?;
    let journal = directory(&root, "journal", create).await?;
    let year = directory(&journal, &iso[0..4], create).await?;

    directory(&year, &iso[5..7], create).await
}

async fn directory(
    parent: &FileSystemDirectoryHandle,
    name: &str,
    create: bool,
) -> Result<FileSystemDirectoryHandle, JsValue> {
    let options = FileSystemGetDirectoryOptions::new();
    options.set_create(create);

    JsFuture::from(parent.get_directory_handle_with_options(name, &options))
        .await?
        .dyn_into()
}

async fn file_handle(
    parent: &FileSystemDirectoryHandle,
    name: &str,
    create: bool,
) -> Result<FileSystemFileHandle, JsValue> {
    let options = FileSystemGetFileOptions::new();
    options.set_create(create);

    JsFuture::from(parent.get_file_handle_with_options(name, &options))
        .await?
        .dyn_into()
}

async fn read_file(handle: &FileSystemFileHandle) -> Result<String, JsValue> {
    let file: File = JsFuture::from(handle.get_file()).await?.dyn_into()?;
    let text = JsFuture::from(file.text()).await?;

    Ok(text.as_string().unwrap_or_default())
}

async fn next_handle(iterator: &AsyncIterator) -> Result<Option<FileSystemHandle>, JsValue> {
    let next = JsFuture::from(iterator.next()?).await?;
    let done = Reflect::get(&next, &JsValue::from_str("done"))?
        .as_bool()
        .unwrap_or(true);

    if done {
        return Ok(None);
    }

    let value = Reflect::get(&next, &JsValue::from_str("value"))?;
    Ok(Some(value.dyn_into()?))
}

fn is_not_found(error: &JsValue) -> bool {
    Reflect::get(error, &JsValue::from_str("name"))
        .ok()
        .and_then(|name| name.as_string())
        .is_some_and(|name| name == "NotFoundError")
}
