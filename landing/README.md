# Dainiki landing page

The marketing site for Dainiki, built with Topcoat 0.10 and Tailwind CSS.

## Run it

```sh
cd landing
topcoat dev
```

Open <http://127.0.0.1:3000>.

For a plain build:

```sh
cargo build --release
topcoat asset bundle --release
./target/release/dainiki-landing
```

## Structure

```text
landing/
├── build.rs                  # compiles Tailwind at build time
├── Cargo.toml
├── src/
│   ├── main.rs               # router: layout + pages + assets + fonts
│   ├── landing.rs            # root layout, home page, /app placeholder
│   ├── waitlist.rs           # D1/file waitlist store + POST /waitlist
│   ├── scripts/theme.js      # light/dark theme toggle
│   ├── styles/app.css        # Tailwind theme tokens + dark palette
│   ├── styles/fonts/         # self-hosted Inter + Newsreader
│   └── landing/
│       ├── header.rs
│       ├── hero.rs
│       ├── showcase.rs       # text-only app mockup
│       ├── features.rs
│       ├── markdown.rs
│       ├── manifesto.rs
│       ├── pricing.rs
│       ├── faq.rs
│       └── footer.rs
```

## Deploy

The live landing runs as the Cloudflare Worker `smbl-dainiki` at
<https://dainiki.smbl.dev>, using Workers Static Assets plus the existing
`smbl-wishlist` D1 database.

From `landing/`:

```sh
cargo build --release
topcoat asset bundle --release
node deploy/export.mjs
cd deploy
npx wrangler@latest deploy
```

`deploy/export.mjs` starts the release binary, exports `/` and `/app`, copies
the hashed assets and font stylesheets, and inserts the waitlist status
placeholder. `deploy/src/index.js` handles `POST /waitlist`, D1 inserts, and
status-message injection. `deploy/public/` is generated and gitignored.

## Waitlist

The hero form posts to `POST /waitlist`. The default backend is Cloudflare D1,
using the existing `smbl-wishlist` database and its existing `waitlist` table.
Current subscribers are untouched; new inserts use
`ON CONFLICT(email) DO NOTHING`, so duplicates are reported without creating a
second row.

Create `landing/.env` and fill in the account id, database id, and token:

```sh
cp .env.example .env
# edit .env with the real Cloudflare values
topcoat dev
```

Create the token at <https://dash.cloudflare.com/profile/api-tokens>:

1. **Create Token** → **Custom token**
2. Permission: **Account → D1 → Edit**
3. Account resources: include your account
4. Create it and put the one-time value in `CLOUDFLARE_API_TOKEN`

Wrangler’s OAuth session is separate from a server API token. It is useful for
CLI verification (`wrangler d1 execute ...`), but it is not a stable credential
for a deployed server. Do not commit the token.

The existing table schema is documented in `d1/schema.sql`. The production
database already has it; the file is there for rebuilding the database.
Existing rows are left untouched.

For local development without Cloudflare credentials, use the file backend:

```sh
WAITLIST_BACKEND=file WAITLIST_FILE=data/waitlist.tsv topcoat dev
```

Or test the D1 code path against the mock:

```sh
node scripts/mock-d1.mjs
WAITLIST_BACKEND=d1 \
CLOUDFLARE_ACCOUNT_ID=dev \
CLOUDFLARE_API_TOKEN=dev \
CLOUDFLARE_API_BASE=http://127.0.0.1:8787/client/v4 \
topcoat dev
```

The form includes a honeypot field and server-side email validation. The file
backend writes `data/waitlist.tsv` (email + Unix timestamp), lowercased and
deduplicated; that file is gitignored.

## Notes

- The landing is text-only: the product mockup contains no photos, audio, or
  video. The `src/landing/img/` folder only contains the logo and the generated
  OG image.
- `/app` is a deliberate placeholder until the local-first journal app is
  built.
- Fonts are self-hosted through Topcoat’s font system. No Google Fonts request
  is made at runtime.
- The docs folder is ignored by Git; the root `README.md` is the only tracked
  documentation file.
