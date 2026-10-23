#!/bin/sh
set -eu

: "${DAINIKI_DB_PASSWORD:?DAINIKI_DB_PASSWORD is required}"

# One PostgreSQL instance shared by products. Each product gets its own
# PostgreSQL role and schema; the role's search_path makes its schema the
# default for that product without changing application SQL.
psql -v ON_ERROR_STOP=1 --username "$POSTGRES_USER" --dbname "$POSTGRES_DB" <<SQL
DO \$\$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_roles WHERE rolname = 'dainiki_app') THEN
        CREATE ROLE dainiki_app LOGIN PASSWORD '${DAINIKI_DB_PASSWORD}';
    ELSE
        ALTER ROLE dainiki_app PASSWORD '${DAINIKI_DB_PASSWORD}';
    END IF;
END
\$\$;

CREATE SCHEMA IF NOT EXISTS dainiki AUTHORIZATION dainiki_app;
ALTER ROLE dainiki_app SET search_path = dainiki, public;
GRANT CONNECT ON DATABASE ${POSTGRES_DB} TO dainiki_app;
GRANT USAGE, CREATE ON SCHEMA dainiki TO dainiki_app;
SQL
