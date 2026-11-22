# Publicar Nexa POS en el VPS

Guía para instalar Nexa en un VPS con 8 GB de RAM y 4 vCPU.

**Estado:** la fase S está aprobada en código. El pendiente antes de abrir al público es aplicar las migraciones pendientes en Supabase; las aplica Claude, no el usuario ni un script automático.

**Qué NO queda publicado:** facturación electrónica DIAN. Esta versión es solo para negocios no obligados a emitir facturas electrónicas.

## Cómo queda armado

```text
Internet -> Caddy (HTTPS, puertos 80 y 443) -> Next.js -> API Rust -> Supabase
```

Caddy obtiene y renueva el certificado HTTPS solo. La API y el frontend no quedan expuestos a internet. La base de datos y el inicio de sesión siguen en Supabase.

## Lo que necesitás antes

1. **Un dominio**, con un registro `A` apuntando a la IP del VPS. Por ejemplo, `app.tudominio.com`.
2. **Docker y Docker Compose** en el VPS. Estos pasos suponen Ubuntu o Debian.
3. **Plan Pro de Supabase.** El plan gratis se pausa tras una semana sin uso y no tiene backups.
4. **Supabase > Authentication > URL Configuration:**
   - Site URL: `https://DOMINIO`
   - Redirect URLs: `https://DOMINIO/login/restablecer` y `https://DOMINIO/onboarding`
   Sin estas URLs, recuperar contraseña y confirmar correo no funcionan (el frontend usa `window.location.origin` en `login-screen.tsx` y `register-screen.tsx`).

## Cómo queda el disco

```text
/opt/nexa/              <- clon único de nexa/, rama feat/m18-demo
  backend/
  frontend/
  despliegue/           <- acá se corre docker compose
```

Es un **solo repositorio** (`yospino814-source/nexa`). La carpeta `despliegue/` viaja dentro del repositorio: no hay que copiarla a mano.

## Primer despliegue (orden obligatorio)

Seguí este orden. No lo cambies.

### 1. Firewall

```sh
sudo ufw allow OpenSSH
sudo ufw allow 80/tcp
sudo ufw allow 443
sudo ufw enable
```

### 2. Clonar el repositorio

```sh
sudo mkdir -p /opt/nexa && sudo chown "$USER" /opt/nexa
cd /opt/nexa
git clone -b feat/m18-demo https://github.com/yospino814-source/nexa .
```

### 3. Variables de entorno

```sh
cd /opt/nexa/despliegue
cp .env.ejemplo .env
chmod 600 .env
nano .env
```

Llenás `DATABASE_URL`, `SUPABASE_URL`, `SUPABASE_PUBLISHABLE_KEY` y `DOMINIO`.
`DATABASE_URL` contiene la contraseña de la base: nunca la pegues en un chat ni la subas a git.

### 4. Respaldo previo (si ya hay datos en Supabase)

```sh
export DATABASE_URL=<la-url-del-step-anterior>
./respaldos/respaldar.sh
```

Guardá el archivo `.dump` que genera en un lugar fuera del VPS (disco local, S3, etc.).

### 5. Claude aplica las migraciones

Antes de levantar el código nuevo, Claude revisa y aplica las migraciones SQL pendientes. Nadie más las aplica.

### 6. Arrancar

```sh
cd /opt/nexa/despliegue
docker compose up -d --build
docker compose ps
```

La primera vez tarda varios minutos: compila Rust y Next.js.

### 7. Comprobar

```sh
./verificar.sh DOMINIO
```

---

## Actualizaciones

```sh
cd /opt/nexa && git pull
cd despliegue && ./respaldos/respaldar.sh   # respaldo antes de cualquier cambio
# si hay migraciones nuevas, Claude las aplica aquí
docker compose up -d --build
./verificar.sh DOMINIO
```

**Rollback:** si algo sale mal, volvés al commit anterior y reconstruís:

```sh
cd /opt/nexa && git checkout <hash-del-commit-anterior>
cd despliegue && docker compose up -d --build
```

---

## Respaldos

Los respaldos los hace `pg_dump`, que debe ser de la misma versión mayor que el Postgres de Supabase (consultalo en Supabase > Project Settings > Database).

### Hacer un respaldo

```sh
export DATABASE_URL=$(grep DATABASE_URL .env | cut -d= -f2-)
./respaldos/respaldar.sh
```

Genera `respaldos/salida/nexa-AAAAMMDD-HHMM.dump`. Los archivos con más de 30 días se borran solos. El registro queda en `respaldos/registro.log`.

**Guardá el `.dump` fuera del VPS** (disco local, S3, etc.); si el VPS se pierde, el respaldo se pierde con él.

### Restaurar

```sh
export DATABASE_URL=$(grep DATABASE_URL .env | cut -d= -f2-)
./respaldos/restaurar.sh respaldos/salida/nexa-AAAAMMDD-HHMM.dump
```

El script te pide escribir `RESTAURAR` (en mayúsculas) antes de hacer cualquier cambio.

---

## Comprobar el despliegue

```sh
./verificar.sh DOMINIO
```

El script comprueba, con `✓` o `✗` por punto:

1. `https://DOMINIO/api/v1/health` responde 200.
2. `https://DOMINIO/api/v1/health/db` responde 200. Falla si `DATABASE_URL` está mal.
3. `https://DOMINIO/login` responde 200 y contiene `Nexa`.
4. El certificado HTTPS es válido y le faltan más de 10 días para vencer.
5. `http://DOMINIO` redirige a `https://`.
6. `https://DOMINIO/api/v1/context` sin token devuelve 401, no 200. Si devuelve 200, la API está abierta: fallo grave.
7. `docker compose ps` muestra `api`, `web` y `caddy` en `running` y `api` como `healthy` (se omite si `docker` no está disponible donde se corre el script).

---

## Disco y memoria

El disco del VPS se llena con la caché de builds de Rust. Después de cada actualización:

```sh
docker system df
docker builder prune -f
docker image prune -f
```

El `docker-compose.yml` limita `api` y `web` a 1 GB de RAM cada uno. Ese límite aplica al contenedor en ejecución, no al build. Si el build de Rust muere por falta de memoria, bajá la concurrencia:

```sh
# en docker-compose.yml, en el build de api, agregá:
#   args:
#     CARGO_BUILD_JOBS: "2"
```

El VPS tiene 8 GB de RAM y 4 vCPU.

---

## Problemas comunes

| Síntoma | Qué revisar |
|---|---|
| El dominio no abre o no hay HTTPS | Que el registro `A` ya apunte al VPS y que los puertos 80 y 443 estén abiertos. Ver `docker compose logs caddy`. |
| La API se reinicia | `docker compose logs api`. Casi siempre es `DATABASE_URL`. |
| `/api/v1/health` responde, pero `/health/db` no | La conexión a Supabase. Usar la URL de *Session pooler*. |
| El login devuelve 401 en todo | Supabase debe usar *JWT Signing Keys* (ES256 o RS256). |
| Se llena el disco | `docker system df`, luego `docker builder prune -f` y `docker image prune -f`. Correrlo después de cada actualización. |
| El build de Rust muere | Bajar concurrencia con `CARGO_BUILD_JOBS=2` como build arg. |

---

## Si cambian las variables públicas

`SUPABASE_URL` y `SUPABASE_PUBLISHABLE_KEY` quedan fijadas al compilar el frontend. Si cambian, hay que volver a correr `docker compose up -d --build`.

---

## Nunca en el chat ni en git

`DATABASE_URL`, la clave secreta de Supabase ni ninguna contraseña. Si el usuario las pega accidentalmente en algún lado, se rotan de inmediato en Supabase.
