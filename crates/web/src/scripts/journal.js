const editor = document.querySelector("#entry-editor");
const status = document.querySelector("#entry-status");
const tagList = document.querySelector("#entry-tags");

let wasm = null;
let currentFile = null;
let saveTimer = null;
let saving = false;

function today() {
  const now = new Date();
  const year = String(now.getFullYear());
  const month = String(now.getMonth() + 1).padStart(2, "0");
  const day = String(now.getDate()).padStart(2, "0");

  return { now, year, month, iso: `${year}-${month}-${day}` };
}

function setStatus(text) {
  if (status) status.textContent = text;
}

function updateDateLabels() {
  const { now } = today();
  const day = document.querySelector("#today-day");
  const month = document.querySelector("#today-month");

  if (day) day.textContent = String(now.getDate());
  if (month) {
    month.textContent = now.toLocaleDateString("en", {
      month: "short",
      year: "numeric",
    });
  }
}

async function initWasm() {
  try {
    const module = await import("/wasm/dainiki_wasm.js");
    await module.default();
    wasm = module;
  } catch (error) {
    console.error("WASM failed to load", error);
    setStatus("Local parser failed to load; entries still save");
  }
}

function renderMeta(text, date) {
  if (!wasm) return null;

  try {
    const meta = JSON.parse(wasm.parse_entry_json(text, date));

    if (tagList) {
      tagList.replaceChildren(
        ...meta.tags.map((tag) => {
          const span = document.createElement("span");
          span.className = "font-mono text-ui-label text-secondary/70";
          span.textContent = `#${tag}`;
          return span;
        })
      );
    }

    return meta;
  } catch (error) {
    console.error("parse_entry_json failed", error);
    return null;
  }
}

async function getJournalDir(create) {
  const { year, month } = today();
  const root = await navigator.storage.getDirectory();
  const journal = await root.getDirectoryHandle("journal", { create });
  const yearDir = await journal.getDirectoryHandle(year, { create });

  return yearDir.getDirectoryHandle(month, { create });
}

async function loadEntry() {
  const { iso } = today();

  try {
    const monthDir = await getJournalDir(false);
    currentFile = await monthDir.getFileHandle(`${iso}.md`);
    const file = await currentFile.getFile();
    const text = await file.text();

    editor.value = text;
    const meta = renderMeta(text, iso);
    setStatus(meta ? `Loaded · ${meta.words} words` : "Loaded from this device");
  } catch (error) {
    if (error.name === "NotFoundError") {
      editor.value = "";
      renderMeta("", iso);
      setStatus("New entry — not saved yet");
    } else {
      console.error(error);
      setStatus("Could not open local storage");
    }
  }
}

async function saveEntry() {
  if (saving) return;

  const { iso } = today();
  const text = editor.value;

  if (!text.trim() && !currentFile) {
    setStatus("New entry — not saved yet");
    return;
  }

  saving = true;
  setStatus("Saving…");

  try {
    const monthDir = await getJournalDir(true);
    currentFile = await monthDir.getFileHandle(`${iso}.md`, { create: true });

    const writable = await currentFile.createWritable();
    await writable.write(text);
    await writable.close();

    const meta = renderMeta(text, iso);
    setStatus(meta ? `Saved locally · ${meta.words} words` : "Saved locally");
  } catch (error) {
    console.error(error);
    setStatus("Save failed");
  } finally {
    saving = false;
  }
}

function scheduleSave() {
  clearTimeout(saveTimer);
  setStatus("Unsaved changes");
  saveTimer = setTimeout(() => {
    void saveEntry();
  }, 400);
}

async function main() {
  if (!editor) return;

  updateDateLabels();

  if (!navigator.storage?.getDirectory) {
    setStatus("This browser does not support local file storage");
    editor.disabled = true;
    return;
  }

  await initWasm();
  await loadEntry();

  editor.addEventListener("input", scheduleSave);
  editor.addEventListener("blur", () => {
    void saveEntry();
  });

  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "hidden") {
      void saveEntry();
    }
  });
}

void main();
