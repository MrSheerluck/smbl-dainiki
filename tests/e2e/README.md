# Local e2e tests

Browser tests use Python Playwright. No Node or `node_modules` are required.

## Prerequisites

```sh
python3 -m pip install playwright
python3 -m playwright install chromium
docker compose up -d db rustfs storage-init
```

## Run

Start a mock-billing test server on port 3001 from the repo root:

```sh
wasm-pack build crates/client --target web --out-dir ../../web/client/pkg
PORT=3001 APP_BASE_URL=http://127.0.0.1:3001 \
  AUTH_EMAIL_MODE=file BILLING_MODE=mock \
  DODO_PAYMENTS_WEBHOOK_KEY=whsec_d2hvb2stc2VjcmV0 \
  topcoat dev -p dainiki-web
```

In another terminal:

```sh
DAINIKI_URL=http://127.0.0.1:3001 \
  DAINIKI_WEBHOOK_SECRET=whsec_d2hvb2stc2VjcmV0 \
  python3 -m unittest discover -s tests/e2e -p 'test_*.py' -v
```

Override the URL with `DAINIKI_URL`. The webhook test is skipped when
`DAINIKI_WEBHOOK_SECRET` is unset.

The sync tests need RustFS on `127.0.0.1:9000` with the private
`dainiki-sync` bucket and CORS for `http://127.0.0.1:3001`. `storage-init`
creates both. Do not run code edits while the suite is running; `topcoat dev`
may rebuild and briefly drop the port.

## Coverage

Journal (`test_journal.py`):

- write, autosave, reload, OPFS persistence
- inline editing of past entries while today stays at the top
- local search opening an inline editor with live Markdown preview
- tag counts/filtering and calendar date selection
- light/dark theme persistence
- external-change conflict copies
- export zip, clear OPFS, import zip
- service worker offline shell
- mobile sidebar drawer

Accounts (`test_auth.py`):

- anonymous `/account` redirects to `/signin`
- plan-gated signup, account page, session cookie flags
- signout, anonymous redirect, failed signin, successful signin
- password mismatch and duplicate email errors
- email verification, password reset, email change, password change, deletion
- CSRF rejection for account actions

Encrypted sync (`test_sync.py`):

- sync requires an active plan
- passphrase setup, two-device download, auto-connect after reload, Forget this device
- conflict copies with the remote version canonical
- encrypted deletion/tombstone propagation
- recent-first download (30 entries) and explicit Download all

Billing (`test_billing.py`):

- anonymous `/billing` redirects to `/signup`
- mock checkout activates a plan and shows it on the account page
- unverified accounts cannot subscribe
- checkout requires CSRF
- signed subscription webhook updates state, duplicates are idempotent, and bad
  signatures are rejected

Each journal/sync test uses a fresh browser context, so OPFS starts empty. Auth,
billing, and sync tests create a unique account per run; the local Postgres
volume keeps those rows and RustFS keeps encrypted objects. Clean test accounts
before manual testing if desired.

The sync tests require RustFS on `127.0.0.1:9000` with the `dainiki-sync` bucket
and CORS for `http://127.0.0.1:3001`. `storage-init` creates both.
