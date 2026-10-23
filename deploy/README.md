# Production stack

This is the shared single-VPS layout for SMBL products.

```
Cloudflare edge
      |
Cloudflare Tunnel (outbound only)
      |
   Caddy :80 (internal)
      |                \
 Dainiki :3000       future products
      |
  Postgres :5432 (internal, schema-per-product)
```

No application port is published on the VPS. Only SSH is exposed.

## Layout on the server

```text
/opt/smbl/
├── compose.yaml
├── .env                         # Postgres superuser + per-app schema passwords
├── Caddyfile
├── postgres/init/01-dainiki.sh
├── cloudflared/
│   ├── config.yml
│   └── tunnel.json              # secret, mode 600
├── apps/dainiki/
│   ├── compose.yaml
│   └── .env                     # app secrets, mode 600
└── backups/
```

## First start

```sh
cd /opt/smbl
docker compose pull
docker compose up -d
docker compose ps
```

The Dainiki app is a separate Compose project:

```sh
cd /opt/smbl/apps/dainiki
docker compose pull
docker compose up -d
docker compose ps
```

## Product database isolation

The shared database is `smbl`. Each product gets a dedicated login role and
schema. Dainiki uses:

```text
role:   dainiki_app
schema: dainiki
```

The role runs with `search_path = dainiki, public`, so application migrations
and SQLx queries remain unqualified while data is segregated from future
products. Do not grant product roles access to other product schemas.

## Backups

Install `deploy/backup.sh` on the server and run it from deploy's crontab:

```cron
17 3 * * * /opt/smbl/backup.sh >> /opt/smbl/backups/backup.log 2>&1
```

Restore a dump into a new shared database or directly into `smbl`:

```sh
docker compose -f /opt/smbl/compose.yaml exec -T postgres \
  pg_restore --username postgres --dbname smbl --clean --if-exists \
  < /opt/smbl/backups/smbl-YYYYMMDDTHHMMSSZ.dump
```

Test the restore path before launch.

## Security notes

- SSH is key-only; root login is disabled; UFW allows only port 22.
- Postgres, Caddy, and the app are reachable only over Docker networks.
- Cloudflare Tunnel is outbound-only; Caddy does not publish 80/443.
- Caddy sets security headers and forwards `CF-Connecting-IP` so Topcoat can
  rate-limit and audit real client addresses.
- The app trusts only the `smbl_edge` Docker subnet (`172.30.0.0/24`).
