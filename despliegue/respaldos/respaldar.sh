#!/usr/bin/env bash
# respaldar.sh — Vuelca la base a un archivo .dump local.
# Uso: DATABASE_URL=<url> ./respaldos/respaldar.sh
# Requiere pg_dump de la misma versión mayor que el Postgres de Supabase.
set -euo pipefail

# ── Validaciones ──────────────────────────────────────────────────────────────
if [[ -z "${DATABASE_URL:-}" ]]; then
  echo "ERROR: la variable DATABASE_URL no está definida." >&2
  echo "  Exportala antes de correr este script:" >&2
  echo "  export DATABASE_URL=<url-del-session-pooler>" >&2
  exit 1
fi

# ── Preparar directorio de salida ─────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
SALIDA="$SCRIPT_DIR/salida"
mkdir -p "$SALIDA"

# ── Nombre del archivo ────────────────────────────────────────────────────────
FECHA="$(date +%Y%m%d-%H%M)"
ARCHIVO="$SALIDA/nexa-$FECHA.dump"

# ── Respaldo ──────────────────────────────────────────────────────────────────
echo "Respaldando base de datos..."
if pg_dump --no-owner --no-acl -Fc "$DATABASE_URL" -f "$ARCHIVO"; then
  TAMANO="$(du -sh "$ARCHIVO" | cut -f1)"
  RESULTADO="ok"
  echo "Respaldo guardado en: $ARCHIVO ($TAMANO)"
else
  RESULTADO="error"
  echo "ERROR: pg_dump falló." >&2
  exit 1
fi

# ── Limpiar archivos de más de 30 días ────────────────────────────────────────
find "$SALIDA" -name "*.dump" -mtime +30 -delete
echo "Archivos mayores a 30 días eliminados."

# ── Registrar en log ──────────────────────────────────────────────────────────
LOG="$SCRIPT_DIR/registro.log"
echo "$(date '+%Y-%m-%d %H:%M')  $RESULTADO  ${TAMANO:-?}  $ARCHIVO" >> "$LOG"
echo "Entrada registrada en $LOG."
