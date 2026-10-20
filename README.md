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

## Status

The landing page is built. The local-first journal app is next. The working
product plan lives locally at `docs/plan.md` and is intentionally ignored by
Git. Only this README is tracked from the project root documentation.

## Planned stack

- Rust + [Topcoat](https://github.com/tokio-rs/topcoat)
- SQLite
- Tailwind CSS
- CodeMirror for the Markdown editor
- Cloudflare R2 for encrypted paid sync
- Dodo Payments for billing

## License

MIT
