# Deploy and Host Dainiki on Railway

Dainiki is a quiet, text-only, local-first daily journal for the web. Entries are
plain Markdown files stored in the browser's Origin Private File System. It works
offline, requires no account for local use, and never locks your writing into a
database. Optional end-to-end encrypted sync is available with a paid
subscription.

## About Hosting Dainiki

This template deploys the Rust/Topcoat web app and a Postgres database. The app
serves the journal UI and the optional account, billing, and encrypted-sync APIs.
Postgres stores accounts, sessions, and opaque sync metadata.

Your journal entries stay on your device. No volume is required for the app, and
the server never sees plaintext entries when sync is enabled.

The template pre-fills the Postgres and app variables. After deploying, open the
generated Railway domain. You can start writing immediately without an account. To use encrypted sync, create an account and
verify your email. This template uses `AUTH_EMAIL_MODE=log`, so the verification
link appears in the `dainiki` service's deploy logs. Set `AUTH_EMAIL_MODE=resend`
with a `RESEND_API_KEY` to send real email.

## Common Use Cases

- A private daily journal that syncs across devices with end-to-end encryption
- A self-hosted writing app with plain Markdown files you can export at any time
- A local-first journal for a small group or family
- A base for a paid journaling product with Dodo Payments billing
- Testing the Dainiki client and sync stack on a public URL

## Dependencies for Dainiki Hosting

- The `dainiki` service uses the project's published container image,
  `ghcr.io/mrsheerluck/smbl-dainiki:latest`.
- The `Postgres` service uses Railway's PostgreSQL image.
- Local journaling needs no external services.
- Encrypted sync optionally needs an S3-compatible bucket such as Cloudflare R2.
- Real email optionally needs a Resend API key and a verified sender domain.
- Paid plans optionally need Dodo Payments products and webhook keys.

### Deployment Dependencies

- [Dainiki source](https://github.com/MrSheerluck/smbl-dainiki)
- [Container image](https://github.com/MrSheerluck/smbl-dainiki/pkgs/container/smbl-dainiki)
- [Railway templates](https://docs.railway.com/templates)
- [Resend](https://resend.com)
- [Cloudflare R2](https://developers.cloudflare.com/r2/)
- [Dodo Payments](https://dodopayments.com)

### Implementation Details

| Setting | Value |
| --- | --- |
| App image | `ghcr.io/mrsheerluck/smbl-dainiki:latest` |
| App port | `3000` |
| Healthcheck | `/api/health` |
| Database | Railway PostgreSQL |
| App volume | None; journal data lives in the browser |

App environment variables:

```env
DATABASE_URL=${{Postgres.DATABASE_URL}}
APP_BASE_URL=https://${{RAILWAY_PUBLIC_DOMAIN}}
AUTH_EMAIL_MODE=log
SYNC_STORAGE=disabled
PORT=3000
```

## Optional Configuration

Enable encrypted sync with Cloudflare R2 or any S3-compatible store:

```env
SYNC_STORAGE=s3
R2_ENDPOINT=https://ACCOUNT_ID.r2.cloudflarestorage.com
R2_ACCESS_KEY_ID=...
R2_SECRET_ACCESS_KEY=...
R2_BUCKET=dainiki-sync
R2_REGION=auto
R2_FORCE_PATH_STYLE=true
```

Add a bucket CORS rule for the app origin with `GET`, `PUT`, and `HEAD`, allow
all request headers, and expose `ETag`. Keep the bucket private.

Enable real email:

```env
AUTH_EMAIL_MODE=resend
RESEND_API_KEY=...
EMAIL_FROM=hello@example.com
```

Enable paid subscriptions with the Dodo Payments variables from the project
README. Dodo is the merchant of record and handles tax.

## Why Deploy Dainiki on Railway?

Railway is a singular platform to deploy your infrastructure stack. Railway will
host your infrastructure so you don't have to deal with configuration, while
allowing you to vertically and horizontally scale it.

By deploying Dainiki on Railway, you get a working journal with a managed
Postgres database and a public HTTPS URL in one step. There is no server to
patch for basic use, and the journal remains usable even if the server is
offline.

## Notes

- On Railway's free plan the app sleeps when idle; the first request may be slow.
- Journal entries are stored in the browser, not on the server. Export regularly
  if you want an independent backup.
- The source is open source:
  [MrSheerluck/smbl-dainiki](https://github.com/MrSheerluck/smbl-dainiki).
