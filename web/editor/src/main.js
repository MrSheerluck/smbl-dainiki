import {
  createEditor,
  getEditorText,
  setEditorText,
} from "./editor.js";

const editorHost = document.querySelector("#entry-editor");
const status = document.querySelector("#entry-status");
const tagList = document.querySelector("#entry-tags");
const sidebarTagList = document.querySelector("#tag-list");
const timeline = document.querySelector("#timeline-entries");
const activeFilter = document.querySelector("#active-filter");
const activeFilterLabel = document.querySelector("#active-filter-label");
const clearFilterButton = document.querySelector("#clear-filter");
const calendar = document.querySelector("#calendar");
const todayButton = document.querySelector("#today-button");
const prevDayButton = document.querySelector("#prev-day");
const nextDayButton = document.querySelector("#next-day");
const dayLabel = document.querySelector("#today-day");
const weekdayLabel = document.querySelector("#today-weekday");
const monthLabel = document.querySelector("#today-month");

const todayIso = isoFor(new Date());
let selectedIso = todayIso;
let calendarMonth = monthStart(selectedIso);
let allEntries = [];
let activeTag = null;
let wasm = null;
let currentFile = null;
let saveTimer = null;
let savePromise = null;
let editorView = null;
let loading = false;

function isoFor(date) {
  const year = String(date.getFullYear());
  const month = String(date.getMonth() + 1).padStart(2, "0");
  const day = String(date.getDate()).padStart(2, "0");

  return `${year}-${month}-${day}`;
}

function monthStart(iso) {
  const [year, month] = iso.split("-").map(Number);
  return new Date(year, month - 1, 1, 12);
}

function dateInfo(iso) {
  const date = new Date(`${iso}T12:00:00`);

  return {
    date,
    day: String(date.getDate()),
    weekday: new Intl.DateTimeFormat("en", { weekday: "long" }).format(date),
    month: date.toLocaleDateString("en", { month: "short", year: "numeric" }),
  };
}

function setStatus(text) {
  if (status) status.textContent = text;
}

function updateDateLabels(iso) {
  const info = dateInfo(iso);

  if (dayLabel) dayLabel.textContent = info.day;
  if (weekdayLabel) {
    weekdayLabel.textContent = iso === todayIso ? "Today" : info.weekday;
  }
  if (monthLabel) monthLabel.textContent = info.month;
  if (todayButton) todayButton.hidden = iso === todayIso;
  if (nextDayButton) nextDayButton.disabled = iso === todayIso;
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

function parseMeta(text, iso) {
  if (wasm) {
    try {
      return JSON.parse(wasm.parse_entry_json(text, iso));
    } catch (error) {
      console.error("parse_entry_json failed", error);
    }
  }

  const excerpt =
    text
      .split("\n")
      .map((line) => line.trim())
      .find((line) => line.length > 0 && !line.startsWith("#")) ?? "";

  return {
    date: iso,
    excerpt,
    tags: [],
    words: text.trim() ? text.trim().split(/\s+/).length : 0,
  };
}

function renderTags(tags) {
  if (!tagList) return;

  tagList.replaceChildren(
    ...tags.map((tag) => {
      const span = document.createElement("span");
      span.className = "font-mono text-ui-label text-secondary/70";
      span.textContent = `#${tag}`;
      return span;
    })
  );
}

function renderEditorMeta(text, iso) {
  const meta = parseMeta(text, iso);
  renderTags(meta.tags);
  return meta;
}

async function getJournalDir(iso, create) {
  const [year, month] = iso.split("-");
  const root = await navigator.storage.getDirectory();
  const journal = await root.getDirectoryHandle("journal", { create });
  const yearDir = await journal.getDirectoryHandle(year, { create });

  return yearDir.getDirectoryHandle(month, { create });
}

async function loadEntry(iso) {
  try {
    const monthDir = await getJournalDir(iso, false);
    currentFile = await monthDir.getFileHandle(`${iso}.md`);
    const file = await currentFile.getFile();
    const text = await file.text();

    loading = true;
    setEditorText(editorView, text);
    loading = false;

    const meta = renderEditorMeta(text, iso);
    setStatus(`Loaded · ${meta.words} words`);
  } catch (error) {
    loading = false;

    if (error.name === "NotFoundError") {
      currentFile = null;
      loading = true;
      setEditorText(editorView, "");
      loading = false;
      renderEditorMeta("", iso);
      setStatus("New entry — not saved yet");
    } else {
      console.error(error);
      setStatus("Could not open local storage");
    }
  }
}

async function doSave() {
  const iso = selectedIso;
  const text = getEditorText(editorView);

  if (!text.trim() && !currentFile) {
    setStatus("New entry — not saved yet");
    return;
  }

  setStatus("Saving…");

  try {
    const monthDir = await getJournalDir(iso, true);
    currentFile = await monthDir.getFileHandle(`${iso}.md`, { create: true });

    const writable = await currentFile.createWritable();
    await writable.write(text);
    await writable.close();

    const meta = renderEditorMeta(text, iso);
    setStatus(`Saved locally · ${meta.words} words`);
    await refreshTimeline();

    if (getEditorText(editorView) !== text) {
      scheduleSave();
    }
  } catch (error) {
    console.error(error);
    setStatus("Save failed");
  }
}

function saveEntry() {
  if (savePromise) return savePromise;

  savePromise = doSave().finally(() => {
    savePromise = null;
  });

  return savePromise;
}

function scheduleSave() {
  clearTimeout(saveTimer);
  setStatus("Unsaved changes");
  saveTimer = setTimeout(() => {
    void saveEntry();
  }, 400);
}

function renderEntry(entry) {
  const info = dateInfo(entry.iso);

  const article = document.createElement("article");
  article.className = "flex gap-3 sm:gap-5";

  const dateColumn = document.createElement("div");
  dateColumn.className = "w-14 shrink-0 pt-1 text-right";

  const day = document.createElement("span");
  day.className =
    "block font-display text-[1.9rem] leading-none text-on-surface/70 sm:text-[2.2rem]";
  day.textContent = info.day;

  const weekday = document.createElement("span");
  weekday.className =
    "mt-1 block whitespace-nowrap font-ui text-[8px] uppercase tracking-wide text-on-surface-variant/70 sm:text-[9px]";
  weekday.textContent = info.weekday;

  const month = document.createElement("span");
  month.className =
    "mt-0.5 block font-ui text-[8px] text-on-surface-variant/50 sm:text-[9px]";
  month.textContent = info.month;

  dateColumn.append(day, weekday, month);

  const rail = document.createElement("div");
  rail.className =
    "relative min-w-0 flex-1 border-l border-outline-variant/25 pl-4 sm:pl-6";

  const dot = document.createElement("span");
  dot.className =
    "absolute -left-1 top-7 h-1.5 w-1.5 rounded-full bg-outline-variant";

  const link = document.createElement("a");
  link.href = "#entry-editor";
  link.className =
    "block rounded-xl border border-outline-variant/40 bg-surface-container-lowest/80 p-5 shadow-sm transition-colors hover:border-outline focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-primary/40 sm:p-6";
  link.setAttribute(
    "aria-label",
    `Open the entry for ${info.weekday}, ${info.month} ${info.day}`
  );

  const excerpt = document.createElement("p");
  excerpt.className = "font-journal text-journal text-on-surface";
  excerpt.textContent = entry.meta.excerpt || "Empty entry";

  const metaRow = document.createElement("div");
  metaRow.className =
    "mt-4 flex flex-wrap items-center justify-between gap-3";

  const tags = document.createElement("div");
  tags.className = "flex flex-wrap items-center gap-3";

  for (const tag of entry.meta.tags) {
    const tagLabel = document.createElement("span");
    tagLabel.className = "font-mono text-ui-label text-secondary/70";
    tagLabel.textContent = `#${tag}`;
    tags.append(tagLabel);
  }

  const words = document.createElement("span");
  words.className = "font-ui text-[10px] text-on-surface-variant/50";
  words.textContent = `${entry.meta.words} words`;

  metaRow.append(tags, words);
  link.append(excerpt, metaRow);

  link.addEventListener("click", (event) => {
    event.preventDefault();
    void selectDate(entry.iso);
  });

  rail.append(dot, link);
  article.append(dateColumn, rail);

  return article;
}

function renderTagList(entries) {
  if (!sidebarTagList) return;

  const counts = new Map();

  for (const entry of entries) {
    for (const tag of entry.meta.tags) {
      counts.set(tag, (counts.get(tag) ?? 0) + 1);
    }
  }

  if (activeTag && !counts.has(activeTag)) {
    activeTag = null;
  }

  const tags = [...counts.entries()]
    .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
    .slice(0, 8);

  if (tags.length === 0) {
    const empty = document.createElement("p");
    empty.className = "px-2 py-1 font-ui text-[10px] text-on-surface-variant/50";
    empty.textContent = "No tags yet.";
    sidebarTagList.replaceChildren(empty);
  } else {
    sidebarTagList.replaceChildren(
      ...tags.map(([tag, count]) => {
        const button = document.createElement("button");
        button.type = "button";
        button.className =
          activeTag === tag
            ? "flex w-full items-center justify-between rounded bg-surface-container-high px-2 py-1 text-left font-ui text-ui-label text-on-surface"
            : "flex w-full items-center justify-between rounded px-2 py-1 text-left font-ui text-ui-label text-on-surface-variant transition-colors hover:bg-surface-container-high hover:text-on-surface";

        button.addEventListener("click", () => {
          setActiveTag(activeTag === tag ? null : tag);
        });

        const label = document.createElement("span");
        label.className = "flex items-center gap-1.5";

        const hash = document.createElement("span");
        hash.className = "font-mono text-secondary/70";
        hash.textContent = "#";

        const name = document.createElement("span");
        name.textContent = tag;

        label.append(hash, name);

        const countLabel = document.createElement("span");
        countLabel.className = "font-ui text-[10px] text-on-surface-variant/60";
        countLabel.textContent = String(count);

        button.append(label, countLabel);
        return button;
      })
    );
  }

  if (activeFilter) activeFilter.hidden = !activeTag;
  if (activeFilterLabel) {
    activeFilterLabel.textContent = activeTag ? `#${activeTag}` : "";
  }
}

function setActiveTag(tag) {
  activeTag = tag;
  renderTagList(allEntries);
  renderTimeline(allEntries);

  if (tag) {
    timeline?.scrollIntoView({ behavior: "smooth", block: "start" });
  }
}

function renderCalendar(entries) {
  if (!calendar) return;

  const year = calendarMonth.getFullYear();
  const month = calendarMonth.getMonth();
  const entrySet = new Set(entries.map((entry) => entry.iso));

  const header = document.createElement("div");
  header.className = "flex items-center justify-between px-1 pb-1";

  const label = document.createElement("span");
  label.className = "font-ui text-[10px] font-semibold text-on-surface";
  label.textContent = calendarMonth.toLocaleDateString("en", {
    month: "long",
    year: "numeric",
  });

  const controls = document.createElement("div");
  controls.className = "flex items-center gap-1 text-on-surface-variant";

  const previous = document.createElement("button");
  previous.type = "button";
  previous.className = "px-1 text-xs leading-none hover:text-on-surface";
  previous.setAttribute("aria-label", "Previous month");
  previous.textContent = "‹";
  previous.addEventListener("click", () => {
    calendarMonth = new Date(year, month - 1, 1, 12);
    renderCalendar(allEntries);
  });

  const next = document.createElement("button");
  next.type = "button";
  next.className = "px-1 text-xs leading-none hover:text-on-surface";
  next.setAttribute("aria-label", "Next month");
  next.textContent = "›";
  next.addEventListener("click", () => {
    calendarMonth = new Date(year, month + 1, 1, 12);
    renderCalendar(allEntries);
  });

  controls.append(previous, next);
  header.append(label, controls);

  const weekdays = document.createElement("div");
  weekdays.className =
    "grid grid-cols-7 gap-1 text-center font-ui text-[9px] text-on-surface-variant";

  for (const day of ["M", "T", "W", "T", "F", "S", "S"]) {
    const cell = document.createElement("span");
    cell.textContent = day;
    weekdays.append(cell);
  }

  const grid = document.createElement("div");
  grid.className =
    "grid grid-cols-7 gap-1 pt-1 text-center font-ui text-[10px]";

  const first = new Date(year, month, 1, 12);
  const offset = (first.getDay() + 6) % 7;
  const daysInMonth = new Date(year, month + 1, 0, 12).getDate();

  for (let blank = 0; blank < offset; blank += 1) {
    grid.append(document.createElement("span"));
  }

  for (let day = 1; day <= daysInMonth; day += 1) {
    const iso = `${year}-${String(month + 1).padStart(2, "0")}-${String(day).padStart(2, "0")}`;
    const isFuture = iso > todayIso;
    const isSelected = iso === selectedIso;
    const hasEntry = entrySet.has(iso);

    const button = document.createElement("button");
    button.type = "button";
    button.textContent = String(day);
    button.setAttribute("aria-label", iso);
    button.setAttribute("aria-pressed", String(isSelected));

    const classes = [
      "relative flex h-6 w-6 items-center justify-center rounded transition-colors",
    ];

    if (isFuture) {
      classes.push("cursor-default text-on-surface-variant/25");
      button.disabled = true;
    } else if (isSelected) {
      classes.push("bg-primary text-on-primary hover:bg-primary");
    } else {
      classes.push("text-on-surface-variant hover:bg-surface-container");
      if (iso === todayIso) classes.push("font-semibold text-primary");
    }

    if (hasEntry && !isSelected) {
      classes.push(
        "after:absolute after:bottom-0.5 after:left-1/2 after:h-1 after:w-1 after:-translate-x-1/2 after:rounded-full after:bg-primary"
      );
    }

    button.className = classes.join(" ");

    if (!isFuture) {
      button.addEventListener("click", () => {
        void selectDate(iso);
      });
    }

    grid.append(button);
  }

  calendar.replaceChildren(header, weekdays, grid);
}

function renderTimeline(entries) {
  if (!timeline) return;

  const visible = entries.filter(
    (entry) =>
      entry.iso !== selectedIso &&
      (!activeTag || entry.meta.tags.includes(activeTag))
  );

  if (visible.length === 0) {
    const empty = document.createElement("p");
    empty.className = "font-ui text-ui-label text-on-surface-variant/60";
    empty.textContent = activeTag
      ? `No entries tagged #${activeTag}.`
      : "No older entries yet.";
    timeline.replaceChildren(empty);
  } else {
    timeline.replaceChildren(...visible.map(renderEntry));
  }

  if (activeFilter) activeFilter.hidden = !activeTag;
  if (activeFilterLabel) {
    activeFilterLabel.textContent = activeTag ? `#${activeTag}` : "";
  }
}

async function listEntries() {
  const entries = [];

  try {
    const root = await navigator.storage.getDirectory();
    let journal;

    try {
      journal = await root.getDirectoryHandle("journal");
    } catch (error) {
      if (error.name === "NotFoundError") return entries;
      throw error;
    }

    for await (const yearHandle of journal.values()) {
      if (yearHandle.kind !== "directory" || !/^\d{4}$/.test(yearHandle.name)) {
        continue;
      }

      for await (const monthHandle of yearHandle.values()) {
        if (monthHandle.kind !== "directory" || !/^\d{2}$/.test(monthHandle.name)) {
          continue;
        }

        for await (const fileHandle of monthHandle.values()) {
          if (fileHandle.kind !== "file" || !fileHandle.name.endsWith(".md")) {
            continue;
          }

          const iso = fileHandle.name.slice(0, -3);
          if (!/^\d{4}-\d{2}-\d{2}$/.test(iso)) continue;

          const date = new Date(`${iso}T12:00:00`);
          if (Number.isNaN(date.getTime())) continue;

          const file = await fileHandle.getFile();
          const text = await file.text();
          const meta = parseMeta(text, iso);

          entries.push({ iso, text, meta });
        }
      }
    }
  } catch (error) {
    console.error("Could not scan the journal", error);
  }

  entries.sort((a, b) => b.iso.localeCompare(a.iso));
  return entries;
}

async function refreshTimeline() {
  const entries = await listEntries();
  allEntries = entries;
  renderTagList(entries);
  renderTimeline(entries);
  renderCalendar(entries);
}

function shiftDate(days) {
  const date = new Date(`${selectedIso}T12:00:00`);
  date.setDate(date.getDate() + days);
  const iso = isoFor(date);

  if (iso > todayIso) return;
  void selectDate(iso);
}

async function selectDate(iso) {
  if (iso === selectedIso) return;

  await saveEntry();
  selectedIso = iso;
  calendarMonth = monthStart(iso);
  updateDateLabels(iso);
  await loadEntry(iso);
  await refreshTimeline();

  if (editorView) {
    editorView.focus();
    editorHost.scrollIntoView({ behavior: "smooth", block: "center" });
  }
}

async function main() {
  if (!editorHost) return;

  updateDateLabels(selectedIso);

  if (!navigator.storage?.getDirectory) {
    setStatus("This browser does not support local file storage");
    return;
  }

  editorView = createEditor(editorHost, () => {
    if (!loading) scheduleSave();
  });

  await initWasm();
  await loadEntry(selectedIso);
  await refreshTimeline();

  todayButton?.addEventListener("click", () => {
    void selectDate(todayIso);
  });

  clearFilterButton?.addEventListener("click", () => {
    setActiveTag(null);
  });

  prevDayButton?.addEventListener("click", () => {
    shiftDate(-1);
  });

  nextDayButton?.addEventListener("click", () => {
    shiftDate(1);
  });

  document.addEventListener("visibilitychange", () => {
    if (document.visibilityState === "hidden") {
      void saveEntry();
    }
  });
}

void main();
