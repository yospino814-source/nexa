#!/usr/bin/env bash
# verificar.sh — Comprueba que el despliegue en el VPS está sano.
# Uso: ./verificar.sh <dominio>
# Ejemplo: ./verificar.sh app.ejemplo.com
set -uo pipefail

# ── Argumento ─────────────────────────────────────────────────────────────────
if [[ $# -lt 1 ]]; then
  echo "Uso: $0 <dominio>" >&2
  echo "" >&2
  echo "Ejemplo: $0 app.ejemplo.com" >&2
  exit 1
fi

DOMINIO="$1"
FALLOS=0

pass() { echo "✓ $1"; }
fail() { echo "✗ $1"; FALLOS=$((FALLOS + 1)); }

echo "Verificando https://$DOMINIO ..."
echo ""

# ── 1. Health de la API ───────────────────────────────────────────────────────
HTTP=$(curl --max-time 10 -o /dev/null -s -w "%{http_code}" "https://$DOMINIO/api/v1/health" || true)
if [[ "$HTTP" == "200" ]]; then
  pass "1. /api/v1/health responde 200"
else
  fail "1. /api/v1/health respondió $HTTP (esperado 200)"
fi

# ── 2. Health de la base ──────────────────────────────────────────────────────
HTTP=$(curl --max-time 10 -o /dev/null -s -w "%{http_code}" "https://$DOMINIO/api/v1/health/db" || true)
if [[ "$HTTP" == "200" ]]; then
  pass "2. /api/v1/health/db responde 200"
else
  fail "2. /api/v1/health/db respondió $HTTP — probablemente DATABASE_URL está mal"
fi

# ── 3. Página de login con texto Nexa ────────────────────────────────────────
BODY=$(curl --max-time 10 -s "https://$DOMINIO/login" || true)
HTTP=$(curl --max-time 10 -o /dev/null -s -w "%{http_code}" "https://$DOMINIO/login" || true)
if [[ "$HTTP" == "200" ]] && echo "$BODY" | grep -q "Nexa"; then
  pass "3. /login responde 200 y contiene 'Nexa'"
elif [[ "$HTTP" != "200" ]]; then
  fail "3. /login respondió $HTTP (esperado 200)"
else
  fail "3. /login respondió 200 pero no contiene 'Nexa'"
fi

# ── 4. Certificado HTTPS válido y >10 días para vencer ───────────────────────
EXPIRY=$(echo | openssl s_client -servername "$DOMINIO" -connect "$DOMINIO:443" 2>/dev/null \
  | openssl x509 -noout -enddate 2>/dev/null | cut -d= -f2 || true)
if [[ -n "$EXPIRY" ]]; then
  EXPIRY_EPOCH=$(date -d "$EXPIRY" +%s 2>/dev/null || date -j -f "%b %d %T %Y %Z" "$EXPIRY" +%s 2>/dev/null || true)
  NOW_EPOCH=$(date +%s)
  DIAS_REST=$(( (EXPIRY_EPOCH - NOW_EPOCH) / 86400 ))
  if [[ $DIAS_REST -gt 10 ]]; then
    pass "4. Certificado HTTPS válido, vence en $DIAS_REST días"
  else
    fail "4. Certificado HTTPS vence en $DIAS_REST días (mínimo 10)"
  fi
else
  fail "4. No se pudo leer el certificado HTTPS"
fi

# ── 5. HTTP redirige a HTTPS ──────────────────────────────────────────────────
REDIRECT=$(curl --max-time 10 -o /dev/null -s -w "%{redirect_url}" "http://$DOMINIO" || true)
if echo "$REDIRECT" | grep -q "^https://"; then
  pass "5. http://$DOMINIO redirige a https://"
else
  fail "5. http://$DOMINIO no redirigió a https:// (destino: '$REDIRECT')"
fi

# ── 6. /api/v1/context sin token devuelve 401 ────────────────────────────────
HTTP=$(curl --max-time 10 -o /dev/null -s -w "%{http_code}" "https://$DOMINIO/api/v1/context" || true)
if [[ "$HTTP" == "401" ]]; then
  pass "6. /api/v1/context sin token devuelve 401"
elif [[ "$HTTP" == "200" ]]; then
  fail "6. FALLO GRAVE: /api/v1/context sin token devolvió 200 — la API está abierta"
else
  fail "6. /api/v1/context sin token devolvió $HTTP (esperado 401)"
fi

# ── 7. docker compose ps (opcional, solo si docker está disponible) ───────────
echo ""
if command -v docker &>/dev/null; then
  SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
  if docker compose -f "$SCRIPT_DIR/docker-compose.yml" ps --format json 2>/dev/null | \
      python3 -c "
import sys, json
rows = [json.loads(l) for l in sys.stdin if l.strip()]
names = {r.get('Name','') for r in rows}
states = {r.get('Name',''): r.get('State','') for r in rows}
health = {r.get('Name',''): r.get('Health','') for r in rows}
ok = True
for svc in ['api','web','caddy']:
  matched = [n for n in names if svc in n]
  if not matched:
    print(f'  sin contenedor para {svc}'); ok = False; continue
  n = matched[0]
  if states.get(n) != 'running':
    print(f'  {svc} no está running (estado: {states.get(n)})'); ok = False
  if svc == 'api' and health.get(n) != 'healthy':
    print(f'  api no está healthy (health: {health.get(n)})'); ok = False
sys.exit(0 if ok else 1)
" 2>/dev/null; then
    pass "7. docker compose: api, web y caddy running; api healthy"
  else
    fail "7. docker compose ps: algún servicio no está running/healthy"
  fi
else
  echo "  (aviso) docker no disponible en esta máquina: punto 7 omitido"
fi

# ── Resultado final ───────────────────────────────────────────────────────────
echo ""
if [[ $FALLOS -eq 0 ]]; then
  echo "Todo OK. El despliegue parece sano."
  exit 0
else
  echo "$FALLOS punto(s) fallaron. Revisá los ✗ de arriba."
  exit 1
fi
