# Deploy Dainiki on Railway

The one-click template provisions two services:

- `dainiki` - the Rust/Topcoat web app, from the published
  `ghcr.io/mrsheerluck/smbl-dainiki:latest` image
- `Postgres` - accounts, sessions, and encrypted sync metadata

The journal itself stays in the browser's Origin Private File System. Postgres is
only needed for accounts and optional paid sync; local journaling works without
signing up.

## One-click deploy

[![Deploy on Railway](https://railway.com/button.svg)](https://railway.com/new/template/dainiki-1?utm_medium=integration&utm_source=button&utm_campaign=dainiki&referralCode=07teCa)

The button includes the project's Railway referral code. New Railway accounts
get $20 in credits, and Dainiki receives a referral commission at no extra cost
to you. You can also [sign up with the referral link directly](https://railway.com/?referralCode=07teCa).

Railway pays a 15% kickback on usage from published templates, plus a 10%
support bonus when questions in the Template Queue are answered. Referred
signups pay 15% of their invoices for the first 12 months.

## After deploying

1. Open the generated Railway domain. The template pre-fills the Postgres and
   app variables; leave them as-is unless you are adding optional services.
2. Use the journal immediately, or create an account for sync.
3. Verification emails use `AUTH_EMAIL_MODE=log`, so the verification link is
   printed in the `dainiki` service's deploy logs.
4. Sync and billing are off by default. The app works fully without them.

To receive real email, set `AUTH_EMAIL_MODE=resend` and `RESEND_API_KEY`, and
verify `EMAIL_FROM` in Resend.

To enable end-to-end encrypted sync, add Cloudflare R2 credentials:

```sh
SYNC_STORAGE=s3
R2_ENDPOINT=https://<account-id>.r2.cloudflarestorage.com
R2_ACCESS_KEY_ID=...
R2_SECRET_ACCESS_KEY=...
R2_BUCKET=dainiki-sync
R2_REGION=auto
R2_FORCE_PATH_STYLE=true
```

Add a CORS rule for the app origin with `GET`, `PUT`, and `HEAD`, allow all
request headers, and expose `ETag`. Keep the bucket private.

To enable paid plans, add the Dodo Payments variables described in the root
`README.md`.

## Maintaining the template

The published template is at
<https://railway.com/deploy/dainiki-1> (template code `dainiki-1`). Its marketplace
overview is kept in [`deploy/railway/template.md`](railway/template.md).

The app image uses the `latest` tag, which the `build-image` workflow updates on
every push to `main`. Code changes therefore reach new template deployments
without republishing it. Republish only when service settings, variables, or the
marketplace listing change:

```sh
railway templates publish dainiki-1 \
  --category Other \
  --description "Deploy and host Dainiki, a local-first Markdown journal, with Postgres." \
  --readme-file deploy/railway/template.md \
  --image https://dainiki.smbl.dev/og.png
```

Template settings:

- App source: `ghcr.io/mrsheerluck/smbl-dainiki:latest`, built by the
  `build-image` GitHub Actions workflow on every push to `main`.
- App healthcheck: `/api/health`.
- App variables:
  - `DATABASE_URL=${{Postgres.DATABASE_URL}}`
  - `APP_BASE_URL=https://${{RAILWAY_PUBLIC_DOMAIN}}`
  - `AUTH_EMAIL_MODE=log`
  - `SYNC_STORAGE=disabled`
  - `PORT=3000`
- Public networking on port `3000`.
- No volume is required; journal entries live in the browser.

After changing service settings or variables, update the published template in
the Railway template editor. Regenerating from the demo project also works, but
it creates a new template code, so the README button must be updated too. Keep
the `referralCode` parameter in the README button so signups stay attributed.
