# Publicar Nexa POS en el VPS

Guía para instalar Nexa en un VPS con 8 GB de RAM y 4 vCPU.

**Estado:** esta configuración no se ha construido completa todavía. Se validó el archivo `docker-compose.yml`, pero el build de prueba se detuvo porque se llenó el disco del computador local. La primera construcción real se hace en el VPS.

**No publicar para clientes todavía.** Antes hay que aprobar la fase 3 y aplicar las migraciones pendientes en Supabase. Ver `antigravity/REVISION-CLAUDE.md`, sección 10.

## Cómo queda armado

```text
Internet -> Caddy (HTTPS, puertos 80 y 443) -> Next.js -> API Rust -> Supabase
```

Caddy obtiene y renueva el certificado HTTPS solo. La API y el frontend no quedan expuestos a internet. La base de datos y el inicio de sesión siguen en Supabase.

## Lo que necesitás antes

1. **Un dominio**, con un registro `A` apuntando a la IP del VPS. Por ejemplo, `app.tudominio.com`.
2. **Docker y Docker Compose** en el VPS. Estos pasos suponen Ubuntu o Debian.
3. **Plan Pro de Supabase.** El plan gratis se pausa tras una semana sin uso y no tiene backups.
4. **Una URL de sitio en Supabase:** en *Authentication > URL Configuration*, agregá `https://app.tudominio.com`.

## Instalación

### 1. Firewall

```sh
sudo ufw allow OpenSSH
sudo ufw allow 80/tcp
sudo ufw allow 443
sudo ufw enable
```

### 2. Código

La carpeta debe quedar así, porque `docker-compose.yml` busca los proyectos a su lado:

```text
/opt/nexa/
  backend/
  frontend/
  despliegue/
```

```sh
sudo mkdir -p /opt/nexa && sudo chown "$USER" /opt/nexa
cd /opt/nexa
git clone -b RAMA https://github.com/yospino814-source/nexa .
```

Es un solo repositorio (`nexa/`). `RAMA` es la rama aprobada y el repositorio es `yospino814-source/nexa`. La carpeta `despliegue/` viaja dentro del repositorio: no hay que copiarla a mano una vez que M19.1 la commitee.

### 3. Variables

```sh
cd /opt/nexa/despliegue
cp .env.ejemplo .env
chmod 600 .env
nano .env
```

`DATABASE_URL` tiene la contraseña de la base. Nunca la pegues en un chat ni la subas a git.

### 4. Arrancar

```sh
docker compose up -d --build
docker compose ps
```

La primera vez tarda varios minutos, porque compila Rust y Next.js.

### 5. Comprobar

```sh
curl https://app.tudominio.com/api/v1/health
curl https://app.tudominio.com/api/v1/health/db
```

Después, abrí el dominio en el navegador e iniciá sesión.

## Actualizar

```sh
cd /opt/nexa && git pull
cd /opt/nexa/despliegue && docker compose up -d --build
```

Las migraciones nunca se aplican desde aquí: las aplica Claude después de revisarlas, y siempre **antes** de actualizar el código.

## Problemas comunes

| Síntoma | Qué revisar |
|---|---|
| El dominio no abre o no hay HTTPS | Que el registro `A` ya apunte al VPS y que los puertos 80 y 443 estén abiertos. Ver `docker compose logs caddy`. |
| La API se reinicia | `docker compose logs api`. Casi siempre es `DATABASE_URL`. |
| `/api/v1/health` responde, pero `/health/db` no | La conexión a Supabase. Usar la URL de *Session pooler*. |
| El login devuelve 401 en todo | Supabase debe usar *JWT Signing Keys* (ES256 o RS256). |
| Se llena el disco | `docker system df`, y luego `docker builder prune` para borrar caché de builds. |

## Si cambian las variables públicas

`SUPABASE_URL` y `SUPABASE_PUBLISHABLE_KEY` quedan fijadas al compilar el frontend. Si cambian, hay que volver a correr `docker compose up -d --build`.
