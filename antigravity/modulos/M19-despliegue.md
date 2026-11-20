# M19 Despliegue en el VPS

| Dato | Valor |
|---|---|
| Fase | Salida |
| Repositorio | `nexa/` (solo la carpeta `despliegue/`, que ya está en su sitio pero **sin commitear**) |
| Rama | La rama actual de `nexa/` |
| Commits | Uno por tarea: `chore(despliegue): M19.N <resumen>` |
| Migración | Ninguna |
| Reporte | Sección "M19" al final de `antigravity/REPORTE-FASE-S.md` |

**Lo que Antigravity hace aquí es escribir y comprobar archivos de configuración y guías. No se conecta al VPS, no despliega, no aplica migraciones y no pide ni usa claves.** El despliegue real lo hacen Claude y el usuario con estos archivos.

**Dato:** el 2026-11-20 la carpeta `despliegue/` se movió dentro del repositorio, a `nexa/despliegue/`, al lado de `backend/` y `frontend/`, y sus contextos de build ya son `../backend` y `../frontend`. Todavía **no está commiteada**. Este módulo la mete a git, para que el VPS la clone en lugar de copiarla a mano.

## Prompt para Antigravity

```text
Vas a hacer el módulo M19 de Nexa POS. Vos (modelo 3.8) coordinás; el trabajo lo hacen subagentes.

1. Leé completos antigravity/00-LEEME.md, antigravity/INFORME-SALIDA.md (§4),
   antigravity/modulos/M19-despliegue.md y despliegue/LEEME.md.
2. Abrí 4 subagentes (M19.1 a M19.4) y lanzalos TODOS AL MISMO TIEMPO. A cada uno
   dale su tarea, su lista de "Archivos que le pertenecen" y la regla 0 del LEEME.
   Ningún subagente edita un archivo ajeno a su lista.
3. Cuando terminen, uní el trabajo y hacé un commit por tarea.
4. Corré VOS la verificación final (sección "Verificación del coordinador"). Es tu
   trabajo, no el de los subagentes.
5. Agregá la sección "M19" al final de antigravity/REPORTE-FASE-S.md.
6. Subí: git push origin <la rama actual>
7. Detenete. Escribime "M19 listo para revisión".

PROHIBIDO en este módulo: conectarte por SSH a ningún servidor, correr docker build
o docker compose up, aplicar migraciones, correr SQL remoto, leer o escribir
backend/.env o frontend/.env.local, y escribir cualquier clave, token,
contraseña o URL con credenciales en un archivo, un commit o el reporte.
Donde haga falta un secreto, va el nombre de su variable de entorno.
```

---

## M19.1 La carpeta `despliegue/` entra a git

**Archivos que le pertenecen:** `.gitignore` (de la raíz de `nexa/`) y la carpeta `despliegue/`.

**Commit:** `chore(despliegue): M19.1 despliegue dentro del repositorio`

1. La carpeta `despliegue/` ya está en `nexa/despliegue/`, con `LEEME.md`, `docker-compose.yml`, `Caddyfile`, `.env.ejemplo`, `backend.Dockerfile`, `frontend.Dockerfile` y los dos `.dockerignore`. **No la copies ni la muevas.** Solo agregala a git.
2. Comprobá que `docker-compose.yml` usa `context: ../backend` y `context: ../frontend`, y que los comentarios de los Dockerfile dicen lo mismo. Si algo apunta a otro lugar, corregilo. Confirmá en el `LEEME.md` que la estructura que el compose espera es:

```text
/opt/nexa/              <- clon único de nexa/
  backend/
  frontend/
  despliegue/           <- acá se corre docker compose
```

3. En `.gitignore` asegurate de que `despliegue/.env` esté ignorado y que `despliegue/.env.ejemplo` **no** lo esté.

**Prueba del subagente:** `git status` muestra los archivos de `despliegue/` como nuevos y **no** muestra ningún `.env`. `grep -rn "sb_secret\|postgres://\|password" despliegue/` no devuelve nada fuera de `.env.ejemplo`, y ahí solo nombres de variables sin valor.

## M19.2 Respaldo y restauración de la base

**Archivos que le pertenecen:** `despliegue/respaldos/` (nueva) y la sección de respaldos de `despliegue/LEEME.md`. Si M19.1 todavía no copió la carpeta, escribí tus archivos en `despliegue/respaldos/` igual: el coordinador los une.

**Commit:** `chore(despliegue): M19.2 respaldos de la base`

Supabase Pro hace respaldos diarios, pero el negocio necesita poder bajar uno antes de cada migración y guardarlo fuera de Supabase.

1. `respaldos/respaldar.sh`: toma `DATABASE_URL` de una variable de entorno (nunca escrita en el archivo), corre `pg_dump` con `--no-owner --no-acl -Fc`, deja el archivo en `respaldos/salida/nexa-AAAAMMDD-HHMM.dump`, borra los que tengan más de 30 días y escribe una línea en `respaldos/registro.log` con la fecha, el tamaño y el resultado. Si `DATABASE_URL` no está definida, se detiene con un mensaje claro y código 1.
2. `respaldos/restaurar.sh`: recibe la ruta de un `.dump` como primer argumento. **Antes de restaurar, exige confirmación escrita**: pide que el usuario escriba `RESTAURAR` y solo sigue si coincide. Usa `pg_restore --clean --if-exists --no-owner`. Si no recibe argumento, muestra cómo se usa y sale.
3. `respaldos/.gitignore` con `salida/` y `registro.log`.
4. En el `LEEME.md`, la sección "Respaldos": cómo correr cada script, que `pg_dump` debe ser de la misma versión mayor que Postgres de Supabase, y que el `.dump` se guarda fuera del VPS.

**Prueba del subagente:** `bash -n` sobre los dos scripts no da error de sintaxis. Correr `respaldar.sh` sin `DATABASE_URL` sale con código 1 y su mensaje. Correr `restaurar.sh` sin argumento muestra el uso y no toca nada. **No corras un `pg_dump` real.**

## M19.3 Verificación de salud posterior al despliegue

**Archivos que le pertenecen:** `despliegue/verificar.sh` (nuevo) y la sección "Comprobar" de `despliegue/LEEME.md`.

**Commit:** `chore(despliegue): M19.3 verificación posterior al despliegue`

`verificar.sh` recibe el dominio como argumento (`./verificar.sh app.ejemplo.com`) y comprueba, mostrando una línea por punto con ✓ o ✗ y saliendo con código 1 si alguno falla:

1. `https://DOMINIO/api/v1/health` responde 200.
2. `https://DOMINIO/api/v1/health/db` responde 200. Este es el que falla si `DATABASE_URL` está mal.
3. `https://DOMINIO/login` responde 200 y su HTML contiene `Nexa`.
4. El certificado HTTPS es válido y le faltan más de 10 días para vencer.
5. `http://DOMINIO` redirige a `https://`.
6. `https://DOMINIO/api/v1/context` **sin** token responde 401, no 200 ni 500. Si responde 200, la API está abierta: es un fallo grave y el script lo dice así.
7. `docker compose ps` muestra los 3 servicios (`api`, `web`, `caddy`) en estado `running`, y `api` como `healthy`. Este punto se salta, con un aviso, si `docker` no está disponible donde se corre el script.

Sin argumento, muestra cómo se usa y sale con código 1.

**Prueba del subagente:** `bash -n verificar.sh` sin errores; sin argumento muestra el uso; con un dominio que no existe (`no-existe.invalid`) falla con mensajes claros y código 1, sin quedarse colgado (usá `curl --max-time 10`).

## M19.4 Guía de despliegue y actualización, paso por paso

**Archivos que le pertenecen:** `despliegue/LEEME.md`, salvo las secciones de M19.1, M19.2 y M19.3. Para evitar choques, escribí tus secciones nuevas al final del archivo y avisá al coordinador en qué orden van.

**Commit:** `chore(despliegue): M19.4 guía de despliegue`

El `LEEME.md` actual supone cosas que ya no valen: habla de aprobar la fase 3 y de copiar `despliegue/` a mano. Actualizalo:

1. **Estado:** la fase S está aprobada en código. El pendiente es aplicar las migraciones, que las aplica Claude.
2. **Rama exacta:** un solo clon de `nexa/` en la rama aprobada de la fase S. Anotá el nombre real de la rama; el remoto es `https://github.com/yospino814-source/nexa`; `RAMA_FRONTEND` desaparece porque ya no hay dos repositorios.
3. **Orden obligatorio del primer despliegue**, numerado: firewall → clonar → `.env` → **respaldo** → **Claude aplica las migraciones** → `docker compose up -d --build` → `./verificar.sh DOMINIO`. Dejá claro que las migraciones van **antes** de levantar el código nuevo y que nadie más que Claude las aplica.
4. **Actualizaciones:** `git pull` en los dos repositorios, respaldo, migraciones si hay nuevas, `docker compose up -d --build`, `./verificar.sh`. Y cómo volver atrás: `git checkout` al commit anterior y reconstruir.
5. **Disco:** el VPS tiene 8 GB de RAM y 4 vCPU, pero el disco se llena con la caché de builds de Rust. Agregá al cuadro de problemas: `docker system df`, `docker builder prune -f` y `docker image prune -f`, y la recomendación de correrlo después de cada actualización.
6. **Memoria:** el `docker-compose.yml` limita `api` y `web` a 1 GB cada uno. Anotá que la primera compilación de Rust puede necesitar más que eso y que el límite aplica al contenedor en ejecución, no al build; si el build muere por memoria, se baja la concurrencia de cargo con `CARGO_BUILD_JOBS=2`.
7. **Supabase:** en *Authentication > URL Configuration* van `https://DOMINIO` como Site URL y `https://DOMINIO/login/restablecer` y `https://DOMINIO/onboarding` como Redirect URLs. Esto sale de `login-screen.tsx` y `register-screen.tsx`, que usan `window.location.origin`: sin esas URLs, recuperar contraseña y confirmar correo no funcionan.
8. **Qué NO queda publicado:** facturación electrónica DIAN. Agregá una advertencia de una línea: solo negocios no obligados a facturar electrónicamente.
9. **Nunca en el chat ni en git:** `DATABASE_URL`, la clave secreta de Supabase ni contraseñas. Si el usuario las pega en algún lado, se rotan.

**Prueba del subagente:** leé tu propio texto siguiendo los pasos como si fuera la primera vez y revisá que no mande a hacer nada que el módulo prohíbe, que no quede ningún `RAMA_FRONTEND` ni `TU-PROYECTO`, y que ningún comando contenga un secreto.

---

## Verificación del coordinador

Corré **vos**, después del último cambio:

```sh
cd backend
git status --short
grep -rniE "sb_secret|postgres://[^ ]*:[^ ]*@|eyJ[A-Za-z0-9_-]{20,}" despliegue/ || echo "sin secretos"
for f in despliegue/respaldos/respaldar.sh despliegue/respaldos/restaurar.sh despliegue/verificar.sh; do bash -n "$f" && echo "ok $f"; done
docker compose -f despliegue/docker-compose.yml config >/dev/null && echo "compose valido"
./scripts/test.sh "cargo test"
```

- `git status` no debe mostrar ningún `.env`.
- El `grep` debe decir "sin secretos".
- Los tres scripts sin errores de sintaxis.
- `docker compose config` valida el archivo **sin construir nada**. Si Docker no está disponible, anotalo en el reporte y seguí.
- Las pruebas del backend deben seguir pasando: este módulo no cambia código, así que cualquier fallo es una señal de que algo se movió de más.

**No corras `docker compose up` ni `docker build`.** La primera construcción se hace en el VPS.

## Reporte

Sección "M19" al final de `antigravity/REPORTE-FASE-S.md`:
1. Tabla por subagente: tarea, archivos, commit, pruebas.
2. Los choques al unir el `LEEME.md` entre M19.1, M19.2, M19.3 y M19.4, y el orden final de las secciones.
3. La salida completa de tu verificación.
4. Confirmación explícita de que no te conectaste a ningún servidor, no construiste imágenes y no escribiste ningún secreto.
5. Preguntas abiertas con archivo y línea.

## Fuera de alcance

- Conectarse al VPS y desplegar: lo hacen Claude y el usuario.
- Elegir el dominio: decisión del usuario.
- Aplicar migraciones: solo Claude.
- Monitoreo, alertas, CDN, varios servidores y despliegue sin corte.
