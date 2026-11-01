#!/bin/sh
# Corre los tests del backend con Docker: Postgres 17 + shim minimo de Supabase
# + todas las migraciones. No necesita Rust ni Build Tools instalados.
#
#   ./scripts/test.sh            # cargo test
#   ./scripts/test.sh clippy     # cualquier subcomando de cargo
set -e
cd "$(dirname "$0")/.."
ROOT="$(pwd)"
INIT="$(mktemp -d)"
cp scripts/test-db/00_supabase_shim.sql "$INIT/"
for f in supabase/migrations/*.sql; do cp "$f" "$INIT/10_$(basename "$f")"; done

# En Git Bash (Windows) la ruta de mktemp (/tmp/...) no existe para Docker:
# se pasa la ruta de Windows; si no, la base arranca vacia sin migraciones.
INIT_MOUNT="$(cygpath -w "$INIT" 2>/dev/null || echo "$INIT")"

docker network create nexa-test >/dev/null 2>&1 || true
docker rm -f nexa-test-pg >/dev/null 2>&1 || true
MSYS_NO_PATHCONV=1 docker run -d --name nexa-test-pg --network nexa-test \
  -e POSTGRES_PASSWORD=postgres -v "$INIT_MOUNT:/docker-entrypoint-initdb.d:ro" postgres:17 >/dev/null
trap 'docker rm -f nexa-test-pg >/dev/null 2>&1; rm -rf "$INIT"' EXIT

# Espera el fin de la inicializacion; si una migracion falla, el contenedor
# se detiene: se muestra el error y se aborta.
until docker logs nexa-test-pg 2>&1 | grep -q "PostgreSQL init process complete"; do
  if [ "$(docker inspect -f '{{.State.Running}}' nexa-test-pg 2>/dev/null)" != "true" ]; then
    echo "La base de prueba no arranco: una migracion fallo." >&2
    docker logs nexa-test-pg 2>&1 | grep -E "ERROR|psql:" >&2
    exit 1
  fi
  sleep 1
done
until docker exec nexa-test-pg pg_isready -U postgres >/dev/null 2>&1; do sleep 1; done
# Comprobacion: las migraciones se aplicaron (si no, los tests de BD no prueban nada).
docker exec nexa-test-pg psql -U postgres -tAc "select count(*) from public.tenants" >/dev/null || {
  echo "La base de prueba no tiene las migraciones." >&2
  exit 1
}

MSYS_NO_PATHCONV=1 docker run --rm --network nexa-test \
  -v "$ROOT:/src" -v nexa-cargo-reg:/usr/local/cargo/registry -v nexa-target:/target \
  -e CARGO_TARGET_DIR=/target \
  -e DATABASE_URL=postgres://postgres:postgres@nexa-test-pg:5432/postgres \
  -w /src rust:1 sh -c "rustup component add clippy rustfmt >/dev/null 2>&1; cargo ${*:-test}"
