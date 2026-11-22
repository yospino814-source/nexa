#!/usr/bin/env bash
# restaurar.sh — Restaura la base desde un archivo .dump.
# Uso: DATABASE_URL=<url> ./respaldos/restaurar.sh <ruta-al-archivo.dump>
# ADVERTENCIA: borra el contenido actual de la base antes de restaurar.
set -euo pipefail

# ── Validaciones ──────────────────────────────────────────────────────────────
if [[ $# -lt 1 ]]; then
  echo "Uso: DATABASE_URL=<url> $0 <ruta-al-archivo.dump>" >&2
  echo "" >&2
  echo "Ejemplo:" >&2
  echo "  export DATABASE_URL=<url-del-session-pooler>" >&2
  echo "  $0 respaldos/salida/nexa-20260101-0300.dump" >&2
  exit 1
fi

DUMP_FILE="$1"

if [[ ! -f "$DUMP_FILE" ]]; then
  echo "ERROR: no se encuentra el archivo '$DUMP_FILE'." >&2
  exit 1
fi

if [[ -z "${DATABASE_URL:-}" ]]; then
  echo "ERROR: la variable DATABASE_URL no está definida." >&2
  echo "  Exportala antes de correr este script:" >&2
  echo "  export DATABASE_URL=<url-del-session-pooler>" >&2
  exit 1
fi

# ── Confirmación ──────────────────────────────────────────────────────────────
echo ""
echo "ADVERTENCIA: esto va a BORRAR el contenido actual de la base y"
echo "reemplazarlo con el respaldo: $DUMP_FILE"
echo ""
echo -n "Para continuar, escribí RESTAURAR (en mayúsculas): "
read -r CONFIRMACION

if [[ "$CONFIRMACION" != "RESTAURAR" ]]; then
  echo "Cancelado. No se hizo ningún cambio." >&2
  exit 1
fi

# ── Restauración ──────────────────────────────────────────────────────────────
echo ""
echo "Restaurando base de datos desde $DUMP_FILE..."
pg_restore --clean --if-exists --no-owner -d "$DATABASE_URL" "$DUMP_FILE"
echo ""
echo "Restauración completada."
