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
- **Open source**: AGPL-3.0 licensed.
- **Self-hostable**: with built-in email/password auth, SMTP/Resend, and
  optional Cloudflare R2 storage.

## Deploy on Railway

[![Deploy on Railway](https://railway.com/button.svg)](https://railway.com/new/template/dainiki-1?utm_medium=integration&utm_source=button&utm_campaign=dainiki&referralCode=07teCa)

One click provisions the app and a Postgres database on
[Railway](https://railway.com/deploy/dainiki-1). The journal itself still lives
in the browser; Postgres is only used for accounts and optional encrypted sync.
This is a Railway referral link: new accounts get $20 in credits, and Dainiki
earns a commission at no extra cost to you. New to Railway? [Sign up with the
referral link](https://railway.com/?referralCode=07teCa) first, then use the
button above. See [`deploy/railway.md`](deploy/railway.md) for post-deploy setup
and template maintenance.

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

Start Postgres and the local S3-compatible object store first, then build the
client and run the server:

```sh
docker compose up -d db rustfs storage-init
wasm-pack build crates/client --target web --out-dir ../../web/client/pkg
topcoat dev -p dainiki-web
```

Open <http://127.0.0.1:3000>. Entries are stored in the browser's Origin Private
File System as `journal/YYYY/MM/YYYY-MM-DD.md`.

The default `DATABASE_URL` points at the local Compose database. Override it in
a `.env` file when needed; `.env.example` has the defaults. Sqlx migrations run
when the server starts. The app remains usable without an account; auth pages
live at `/signup` and `/signin`.

In local development, verification and reset emails are written to
`data/outbox/` by default. Set `AUTH_EMAIL_MODE=log` to print them instead, or
`AUTH_EMAIL_MODE=resend` with `RESEND_API_KEY` for real delivery. Production
defaults to Resend when `APP_BASE_URL` uses `https://`.

Resend has an official CLI:

```sh
curl -fsSL https://resend.com/install.sh | bash
resend login
resend domains list
resend emails send \
  --from "Dainiki <hello@smbl.dev>" \
  --to delivered@resend.dev \
  --subject "Dainiki test" \
  --text "Resend is working."
```

The sender domain must be verified in Resend. Set `EMAIL_FROM` to a verified
address, then switch `AUTH_EMAIL_MODE=resend`.

Local billing defaults to `BILLING_MODE=mock`, which activates a plan without
taking a payment. For Dodo test or live mode, set:

```sh
BILLING_MODE=dodo
DODO_PAYMENTS_ENVIRONMENT=test_mode   # or live_mode
DODO_PAYMENTS_API_KEY=...
DODO_PAYMENTS_WEBHOOK_KEY=...
DODO_MONTHLY_PRODUCT_ID=...
DODO_YEARLY_PRODUCT_ID=...
```

Create two recurring products in Dodo (monthly $2, yearly $20), then point the
webhook endpoint at `/api/billing/webhook` and subscribe it to the
`subscription.*` events. Dodo is the merchant of record and handles tax.

Encrypted sync uses S3-compatible object storage. Local development runs
RustFS on `127.0.0.1:9000` and creates the private `dainiki-sync` bucket with
CORS for the app origins. The app gets short-lived presigned PUT/GET URLs and
stores only ciphertext in the bucket; Postgres keeps opaque metadata, quota,
and revisions. Configure production Cloudflare R2 with:

```sh
SYNC_STORAGE=s3
R2_ENDPOINT=https://<account-id>.r2.cloudflarestorage.com
R2_ACCESS_KEY_ID=...
R2_SECRET_ACCESS_KEY=...
R2_BUCKET=dainiki-sync
R2_REGION=auto
R2_FORCE_PATH_STYLE=true
```

The endpoint must be reachable by both the server and the browser, because the
presigned URL is signed for that host. Add a bucket CORS rule for the app origin
with `GET`, `PUT`, and `HEAD`, allow all request headers, and expose `ETag`.
Keep the bucket private. A missing or invalid storage configuration leaves local
journaling untouched and disables sync in the UI.

The Dodo CLI can create and manage products, open checkout sessions, and
forward test webhooks to localhost:

```sh
curl -fsSL https://dodopayments.com/install.sh | sh
~/.local/bin/dodo login "$DODO_PAYMENTS_API_KEY" test
~/.local/bin/dodo products list 1
~/.local/bin/dodo wh listen http://127.0.0.1:3000/api/billing/webhook
```

For the live setup, log in with a live key and fill the `DODO_LIVE_*` values.
The app automatically switches to them when
`DODO_PAYMENTS_ENVIRONMENT=live_mode`; otherwise it uses the test values.
The live webhook URL must be the deployed app's public
`/api/billing/webhook` endpoint.

Run the browser suites against a mock-billing server with file email:

```sh
PORT=3001 APP_BASE_URL=http://127.0.0.1:3001 \
  AUTH_EMAIL_MODE=file BILLING_MODE=mock \
  DODO_PAYMENTS_WEBHOOK_KEY=whsec_d2hvb2stc2VjcmV0 \
  topcoat dev -p dainiki-web

DAINIKI_URL=http://127.0.0.1:3001 \
  DAINIKI_WEBHOOK_SECRET=whsec_d2hvb2stc2VjcmV0 \
  python3 -m unittest discover -s tests/e2e -p 'test_*.py' -v
```

Sync tests create a paid mock account, set a sync passphrase, exercise two
devices, verify conflict copies and tombstones, and check recent-first download
limits. They require the local RustFS bucket and its CORS rule.

The full app and database can also run in Docker:

```sh
docker compose --profile full up -d --build
```

This builds the WASM client, the Topcoat server, a Postgres 17 database, and a
local RustFS object store. The app is at <http://127.0.0.1:3000>; `/api/health`
and `/api/ready` are available for container checks. Local verification mail is
stored in the `dainiki-outbox` volume. The RustFS console is at
<http://127.0.0.1:9001>.

## Status

The landing page is live. The local-first journal MVP is complete and awaiting
manual sign-off:

- OPFS-backed Markdown entries, one file per day.
- Rust/WASM client with a cursor-aware live preview editor: the active line
  shows Markdown source, inactive lines render inline.
- Daily editor with previous/next day navigation.
- Timeline built by scanning the stored files.
- Live tag counts, clickable tag filters, and a calendar with entry marks.
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
- First-run onboarding with a persistent-storage request.
- Fifteen local journal browser tests covering storage, editing, search, themes,
  zen mode, explicit data-panel entry management, conflicts, import/export,
  offline mode, and mobile.

Accounts, auth, billing, and encrypted sync are working: email/password signup,
Argon2id hashing, Postgres-backed sessions, email verification, password reset,
CSRF-protected account actions, rate limits, audit logging, account settings,
and Dodo subscription checkout/webhooks. Sync derives an Argon2id key from a
separate passphrase, wraps the master key with a one-time recovery key, encrypts
every manifest and entry with XChaCha20-Poly1305, and stores only ciphertext in
S3-compatible object storage. Presigned URLs keep entry bytes off the app
server, and R2 object keys use opaque HMAC-derived ids rather than dates. The
client syncs recent entries first (30 by default), supports an explicit
“Download all entries” action, detects conflicts with base hashes and
writes local `.conflict-<timestamp>.md` copies that can be reviewed, restored,
downloaded, or deleted in the app, and propagates encrypted entry deletions as
tombstones. An optional diff3 mode automatically merges non-overlapping edits;
overlapping edits still become conflict copies. Paid devices remember the sync key after the first passphrase setup or unlock, so sync stays on automatically; **Forget this device** removes the local key. The full suite has 34 browser tests.

Local billing runs in mock mode by default. Real checkout uses Dodo test or live
mode with two subscription products and a webhook endpoint. No free trial; the
plans are $2/month and $20/year.

The working plan, progress tracker, and code map live in `docs/` and are
intentionally ignored by Git. Only this README is tracked from the project root
documentation.

## Planned stack

- Rust + [Topcoat](https://github.com/tokio-rs/topcoat)
- Postgres 17 + SQLx, run with Docker Compose
- Tailwind CSS
- Rust/WASM browser client (`web-sys`)
- Cloudflare R2 for encrypted paid sync
- Dodo Payments for billing

## License

AGPL-3.0. See [`LICENSE`](LICENSE).
