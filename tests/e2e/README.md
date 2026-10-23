# Local e2e tests

Browser tests use Python Playwright. No Node or `node_modules` are required.

## Prerequisites

```sh
python3 -m pip install playwright
python3 -m playwright install chromium
docker compose up -d db
```

## Run

Start the app from the repo root:

```sh
wasm-pack build crates/client --target web --out-dir ../../web/client/pkg
topcoat dev -p dainiki-web
```

In another terminal:

```sh
python3 -m unittest discover -s tests/e2e -p 'test_*.py' -v
```

Override the URL with:

```sh
DAINIKI_URL=http://127.0.0.1:3000 python3 -m unittest discover -s tests/e2e -p 'test_*.py' -v
```

## Coverage

Journal (`test_journal.py`):

- write, autosave, reload, OPFS persistence
- inline editing of past entries while today stays at the top
- local search opening an inline editor
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

Each journal test uses a fresh browser context, so OPFS starts empty. Auth tests
create a unique account per run; the local Postgres volume keeps those rows.
