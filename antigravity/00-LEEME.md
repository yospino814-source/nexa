# Nexa POS: léeme primero (reglas para todas las fases)

Vas a construir Nexa POS **módulo por módulo**. Cada módulo es un archivo de `antigravity/modulos/`, con su prompt y su especificación exacta. El usuario te pasa el prompt de un módulo; lo hacés, lo verificás, hacés el commit, escribís su sección del reporte y te detenés. Claude revisa al final de cada fase.

**Empezá siempre por `antigravity/modulos/00-INDICE.md`**: tiene el orden, las ramas, las migraciones y el estado de cada módulo. Si un módulo y el documento de su fase no coinciden, manda el módulo.

Los documentos con el prefijo `TERMINADA-` son de fases ya construidas y aprobadas. No se construyen de nuevo; se leen solo para entender el código que ya existe.

## 0. La regla más importante: no decidas

Estos documentos están escritos para que **no tengas que decidir nada**. Por eso:

1. **Lo que no está escrito está prohibido.** No agregues tablas, columnas, endpoints, campos, pantallas, botones, textos, dependencias ni reglas que el documento no pida, aunque parezcan útiles o "lo haría un buen sistema".
2. **Lo que está escrito se hace tal cual:** nombres de tablas, columnas, rutas, códigos de error, permisos, nombres de pruebas y textos de pantalla. No los traduzcas, no los renombres y no los "mejores".
3. **Respeta el orden.** Cuando el documento enumera validaciones o pasos "en este orden", el orden es parte de la especificación.
4. **Si algo no se puede hacer tal cual**, porque choca con el código existente, falta un dato o hay dos instrucciones contradictorias: **detén esa tarea**, escribe la duda en "Preguntas abiertas" del reporte, con el archivo y la línea, y **sigue con la siguiente tarea**. No improvises una solución intermedia.
5. **"Fuera de alcance" significa no hacerlo.** Ni una parte, ni "dejarlo preparado".
6. **No dejes funciones a medias visibles.** Un botón que no funciona, o que el usuario no tiene permiso de usar, no se muestra.
7. **Las diferencias con los diseños mandan a favor del documento.** Si `frontend/design/specs/` muestra algo que el documento de la fase no pide (exportar, IA, filtros extra), no lo hagas. Usa los diseños solo para el aspecto visual.

## 1. Regla legal

El código de Vendix (vendix.online) es público pero **no tiene licencia**. No abras, clones ni copies su repositorio ni ningún archivo suyo (código, SQL, textos, imágenes, íconos o prompts). No uses su marca ni sus nombres propios. Todo lo que necesitas está en estos documentos, escrito con nuestras palabras.

## 2. Reglas de trabajo

1. **Supabase:** no apliques migraciones, no ejecutes SQL remoto y no corras `supabase db push`. Escribe los archivos de migración; Claude los aplica después de revisarlos.
2. **Migraciones:** no modifiques las que ya existen. Cada fase indica el nombre exacto de su archivo. **Una sola migración por entrega.**
3. **Archivos de entorno:**
   - `backend/.env` no se lee ni se edita;
   - `frontend/.env.local` se puede leer, no editar. Con él, el frontend usa la base de **producción**: en ese modo solo verifica que las pantallas carguen, sin crear datos;
   - **nunca** copies valores de entorno, llaves ni secretos a archivos, reportes, commits o mensajes. Cuando una fase necesita una llave (Wompi, Resend, Claude), el código la lee de una variable de entorno y **Claude** la configura. No la pidas ni la busques.
4. **Git:** trabaja en el repositorio único `nexa/` (carpetas `backend` y `frontend`), en la rama que indique el módulo; si no indica ninguna, la rama actual (`git branch --show-current`). Antes de editar corre `git rev-parse --show-toplevel`: debe terminar en `/nexa`; si no, detente. Haz un commit por tarea, con el mensaje `feat(faseN): FN.M <resumen>`. Se sube con `git push origin <rama>` a `yospino814-source/nexa`. Ya no existen `nexa-backend` ni `nexa-frontend`. Sin `reset --hard`, sin rebase, sin borrar ramas y sin `git revert` de tus propios commits: si algo sale mal, corrígelo con un commit nuevo.
5. **Codificación:** todo archivo de código es UTF-8 sin BOM y con saltos de línea LF. **Prohibido** editar código con `Get-Content`, `Set-Content`, `Out-File` o redirecciones de PowerShell: dañan las tildes ("tenés" pasa a "tenÃ©s"). Usa la herramienta de edición de archivos, o Node con `fs.readFileSync(p, "utf8")` y `fs.writeFileSync(p, s, "utf8")`. Antes de cada commit: `grep -rn "Ã\|Â" lib components app` en `frontend` (y el equivalente en `backend/src`) no debe devolver nada.
6. **Procesos:** no mates procesos que no iniciaste. Están ocupados los puertos 3000, 3100 y 8080. Tu frontend va en el **3200**.
7. **Tiempo:** si un comando lleva más de 10 minutos sin terminar, cancélalo y anótalo en el reporte. **No hagas capturas automáticas de pantalla**: Claude revisa la interfaz.
8. **Dependencias:** solo las que la fase permite explícitamente, y justificadas en el reporte.
9. **Ediciones:** cambios puntuales. No reescribas archivos enteros que ya existen.

## 3. Arquitectura y convenciones

| Tema | Convención de Nexa |
|---|---|
| Negocio y sucursal | `tenants` y `branches` |
| Aislamiento | RLS de Postgres con `public.in_scope(tenant_id, branch_id)` o `tenant_id = public.current_tenant_id()`, más el filtro explícito en cada consulta |
| Permisos | Códigos `modulo:accion` en la tabla `permissions`, comprobados con `scope.require_permission("x:y")` |
| Catálogos fijos | `tbl_tipos_maestra`, leídos con `public.tipo_id('CAB','COD')` |
| Estados de tablas nuevas | `text` con `check (status in (...))` |
| Precios | **Sin IVA**; el impuesto se suma encima y se congela por línea |
| Dinero | `numeric(14,2)` y `Decimal`, siempre con `money()` (mitad hacia afuera). Nunca `f64` |
| Cantidades | `numeric(14,4)` |
| Numeración | `sequences::next_tx(tx, scope, branch, doc_type)` |
| Efectos externos | Tabla `outbox`, procesada por un worker |
| Stock | Toda escritura pasa por `inventory::apply_delta_tx` (desde la fase 2) |
| Caja | Todo efectivo que entra o sale pasa por `cash::record_movement_tx` (desde la fase 1) |

### Plantillas obligatorias

**Tabla con sucursal:**

```sql
create table public.X (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  -- columnas del documento
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);
alter table public.X enable row level security;
create policy X_isolation on public.X for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));
grant select, insert, update, delete on public.X to authenticated;
```

**Tabla a nivel de negocio:** igual, sin `branch_id`, y con la política `using (tenant_id = public.current_tenant_id()) with check (tenant_id = public.current_tenant_id())`.

**Claves foráneas entre tablas de negocio:** siempre compuestas, `(tenant_id, x_id) references public.Y (tenant_id, id)`.

**Tipos maestros:**
- cabecera: `on conflict (tima_codigo) where padre_id is null do nothing`;
- hijos: `on conflict (padre_id, tima_codigo) do nothing`.

**Nunca** `grant` a `anon`. **Nunca** políticas `using (true)` en tablas con datos de negocios o de personas.

### Backend
- **Una transacción por operación:** `tenancy::begin_in_branch(...)` → `x_tx(tx, scope, …)` → `commit`. La lógica vive en `_tx`.
- **Permiso al inicio** de cada función pública, justo después de abrir la transacción.
- **Errores:**
  - de la fase 1 en adelante, todo error nuevo usa `error::coded(status, "CODIGO", mensaje, data)`. Los códigos y textos de cada fase están en su documento;
  - 422 es un dato inválido, 409 un conflicto de estado o de concurrencia, 403 la falta de permiso y 404 algo que no existe;
  - **sin `unwrap`** fuera de las pruebas.
- **Idempotencia:** toda operación que cobra, paga o mueve dinero o stock por iniciativa del cliente recibe un `id` UUID generado por el cliente.
  - Si ya existe con el mismo `client_request`, se devuelve el resultado guardado.
  - Si existe con otro cuerpo: 409 `IDEMPOTENCY_CONFLICT`.
- **Bloqueos:** toda validación de saldo, cupo, límite o estado se hace sobre filas bloqueadas con `for update`, dentro de la misma transacción que escribe.
- **Auditoría:** `audit::record_tx(tx, scope, ACCION, tabla, id, before, after, None)`, con los nombres de acción que da cada fase.
- **Endpoints:** cada endpoint queda documentado en `backend/README.md`.

### Seguridad (lecciones de la fase 0)
- **Ningún rol puede darse más poder del que tiene.** Solo `DUENO` asigna o quita `DUENO`, y nadie toca a un dueño salvo otro dueño.
- Las membresías con `active = false` **no** dan acceso.
- Los datos de un negocio nunca son legibles desde otro, ni siquiera "solo lectura".
- Los endpoints públicos (sin sesión) solo devuelven lo que el documento lista. Nunca costos, ids internos de usuarios ni datos de otros clientes.

### Frontend
- **Llamadas:** cada llamada pasa por `api.*` (`lib/api/index.ts`), con su rama mock (`lib/api/mock-<modulo>.ts`) y su rama real. **El modo demo debe funcionar completo**, con datos colombianos realistas en COP.
- **Pantallas:** cargan con `useApi`. Se usan `formatCOP`, `parseCOP`, `MoneyInput`, `StatCard` y `PrintArea`. Las pantallas nuevas se registran en `lib/screens.ts` con el `s(...)` exacto que da el documento, y en el menú lateral solo con el permiso indicado.
- **Permisos:** `useSession().can("x:y")` (desde la fase 1). Lo que el usuario no puede hacer **no se muestra**.
- **Errores:** `friendlyError` muestra el texto de `ERROR_MESSAGES[code]`. Cada fase agrega sus códigos.
- **Textos:** en español con voseo, como el resto de la app.
- **Next.js:** lee `frontend/AGENTS.md`. Esta versión de Next.js trae cambios incompatibles; revisa `node_modules/next/dist/docs/` antes de usar una API de Next.

### Pruebas
- **Una función por tema** en `db_tests.rs`, con el **nombre exacto** que da el documento (por ejemplo, `f1_refunds`), llamada desde `database_integration` en el orden de las tareas.
- **Cada regla [P]** tiene al menos una aserción, con el comentario `// [P] <texto corto de la regla>` encima.
- **Las funciones puras** llevan pruebas unitarias en su propio archivo, con los nombres que da el documento.
- **Las pruebas viejas no se borran.** Si un cambio pedido obliga a ajustar una, explica en el reporte qué cambió y por qué.

## 4. Orden de las fases

| Fase | Documento | Rama | Reporte | Estado |
|---|---|---|---|---|
| 0 | `TERMINADA-fase-0-plataforma.md` (más las correcciones que pidió Claude) | `feat/fase-0-plataforma` | `TERMINADA-REPORTE-FASE-0.md` | **Terminada** |
| 1 | `TERMINADA-fase-1-pos-caja.md` | `feat/fase-1-pos-caja` | `TERMINADA-REPORTE-FASE-1.md` | **Terminada** |
| 2A | `TERMINADA-fase-2-inventario-compras.md` (entrega 2A) | `feat/fase-2a-inventario` | `TERMINADA-REPORTE-FASE-2A.md` | **Terminada** (defecto CSV en M08) |
| 2B | `TERMINADA-fase-2-inventario-compras.md` (entrega 2B) | `feat/fase-2b-compras` | `TERMINADA-REPORTE-FASE-2B.md` | **Terminada** (permisos en M13) |
| 3 | Módulos M01 a M09 (historial en `fase-3-clientes-cartera.md`) | `feat/fase-3-clientes` | `REPORTE-FASE-3.md` | En curso |
| S | `fase-S-salida.md` (alta de negocio, invitaciones, datos del negocio, permisos por rol y reportes) | `feat/fase-s-salida` | `REPORTE-FASE-S.md` | Módulos M10 a M17 |
| 4 | `fase-4-facturacion.md` (espera la decisión D5) | `feat/fase-4-facturacion` | `REPORTE-FASE-4.md` | Bloqueada (D5) |
| 5A | `fase-5-restaurante-domicilios.md` (entrega 5A) | `feat/fase-5a-restaurante` | `REPORTE-FASE-5A.md` | Pospuesta |
| 5B | `fase-5-restaurante-domicilios.md` (entrega 5B) | `feat/fase-5b-domicilios` | `REPORTE-FASE-5B.md` | Pospuesta |
| 6 | `fase-6-tienda-whatsapp.md` | `feat/fase-6-tienda` | `REPORTE-FASE-6.md` | Pospuesta |
| 7 | `fase-7-reportes-ia.md` | `feat/fase-7-reportes` | `REPORTE-FASE-7.md` | F7.1 a F7.3 en la fase S; el resto, pospuesto |
| 8 | `fase-8-opcional.md` | — | — | **No se construye** |

**Prioridad actual:** salir a vender a licorerías y tiendas. La fase 3 se termina con los módulos M01 a M09. Después sigue la fase S, con M10 a M17. La fase 4 espera a que se elija el proveedor de facturación (D5). Las fases 5 y 6 quedan pospuestas, y de la fase 7 solo se construye F7.1 a F7.3, dentro de la fase S. La ruta de salida está en `antigravity/REVISION-CLAUDE.md`.

**Contexto por rol:** `antigravity/flujos-por-rol.md` explica, para cada tipo de usuario, qué necesita y en qué tarea se atiende. Es contexto, no una tarea: si choca con el documento de una fase, manda el de la fase.

**Prompts de diseño:** la carpeta `prompts/` (en la raíz) tiene los prompts visuales con los que se hicieron los diseños. Solo sirven para el aspecto visual (regla 7). Lo que una pantalla hace lo dice el documento de su fase.

Cada **módulo** termina con un alto: escribís su sección del reporte y esperás el prompt del siguiente. Cada **fase** termina con un alto obligatorio (M09 y M17): esperás la aprobación de Claude.

Dentro de cada entrega, el orden es: migración → backend con pruebas → API y mock del frontend → pantallas.

## 5. Verificación (antes de cada commit)

```sh
# Backend: Postgres 17 + shim de Supabase + TODAS las migraciones, en Docker
cd backend && ./scripts/test.sh "fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"

# Frontend
cd frontend && npx tsc --noEmit && npm run lint

# Modo demo (sin backend ni login)
cd frontend && NEXT_PUBLIC_SUPABASE_URL= NEXT_PUBLIC_SUPABASE_PUBLISHABLE_KEY= NEXT_PUBLIC_USE_BACKEND=false npx next dev -p 3200
```

## 6. Reporte de fin de entrega (`antigravity/REPORTE-FASE-<N>.md`)

1. Ramas y commits de cada repositorio (`git log --oneline <base>..HEAD`).
2. Archivos creados y modificados, con una línea por archivo.
3. Las últimas 30 líneas de `test.sh` y la salida de `tsc` y `lint`.
4. La migración y lo que cambia. Recuerda que la aplica Claude.
5. Una tabla con **cada regla [P]** del documento y el nombre de su prueba. Las que faltan, marcadas como "FALTA".
6. Las salidas que pide el documento (por ejemplo, los `grep`).
7. Las dependencias agregadas y su justificación.
8. Las pruebas viejas que se ajustaron y por qué.
9. Preguntas abiertas y tareas detenidas, con el archivo y la línea. Escribe "no lo probé" antes que "funciona" sin haberlo verificado.
