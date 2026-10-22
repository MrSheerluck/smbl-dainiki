# Dainiki

A quiet, text-only, local-first daily journal for the web.

Dainiki keeps every entry as a plain Markdown file on your device. It works
offline, requires no account, and never locks your writing into a database.
Optional end-to-end encrypted sync is available as a paid subscription.

## Principles

- **Local-first**: writing never depends on a server.
- **Text-only**: no images, audio, video, or attachments. Just words.
- **Markdown files**: `YYYY/MM/YYYY-MM-DD.md`, readable in Obsidian, Git, or
  any text editor.
- **No account for local use**: accounts exist only for paid sync.
- **Open source**: MIT licensed.
- **Self-hostable**: with built-in email/password auth, SMTP/Resend, and
  optional Cloudflare R2 storage.

## Landing page

The marketing site is live at <https://dainiki.smbl.dev>. It lives in
`landing/` and is built with Topcoat 0.10 and Tailwind CSS.

```sh
cd landing
topcoat dev
```

Open <http://127.0.0.1:3000>.

## App development

The journal app lives in `crates/web`. Build the Rust/WASM client first, then
run the Topcoat server from the repo root:

```sh
wasm-pack build crates/client --target web --out-dir ../../web/client/pkg
topcoat dev -p dainiki-web
```

Open <http://127.0.0.1:3000>. Entries are stored in the browser's Origin Private
File System as `journal/YYYY/MM/YYYY-MM-DD.md`.

## Status

The landing page is live. The local-first journal is in development:

- OPFS-backed Markdown entries, one file per day.
- Rust/WASM client with a cursor-aware live preview editor: the active line
  shows Markdown source, inactive lines render inline.
- Daily editor with previous/next day navigation.
- Timeline built by scanning the stored files.
- Live tag counts, clickable tag filters, and a calendar with entry dots.
- Local full-text search with `⌘K`, tag matching, snippets, and keyboard navigation.
- Light/dark theme with a persisted preference and system default.
- Conflict copies when a file changes outside the app; local edits are preserved.
- Click any past entry to edit it in place; today's editor stays at the top.
- Export the journal as a Markdown zip; import zip, Markdown files, or a folder
  with conflict-safe date merging.
- No npm, no node_modules, and no client framework. The only JS is the generated
  wasm-bindgen loader and the browser-mandated service worker.
- Installable PWA with an offline app shell.
- Incremental timeline pagination and a mobile sidebar drawer.

Next: local end-to-end tests and final hardening.

The working plan, progress tracker, and code map live in `docs/` and are
intentionally ignored by Git. Only this README is tracked from the project root
documentation.

## Planned stack

- Rust + [Topcoat](https://github.com/tokio-rs/topcoat)
- SQLite
- Tailwind CSS
- Rust/WASM browser client (`web-sys`)
- Cloudflare R2 for encrypted paid sync
- Dodo Payments for billing

## License

MIT
