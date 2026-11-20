# M12 Datos del negocio y sucursales (S3)

| Dato | Valor |
|---|---|
| Fase | S (salida) |
| Repositorios | `nexa-backend` y `nexa-frontend` |
| Rama | `feat/fase-s-salida` |
| Migración | Agrega **al final** de `20261109000100_salida.sql` el bloque `-- S3 Negocio y sucursales`, con el SQL de este módulo. |
| Commit | `feat(fase-s): S3 datos del negocio y sucursales` |
| Reporte | Sección "M12" de `antigravity/REPORTE-FASE-S.md` |

## Prompt para Antigravity

```text
Vas a hacer el módulo M12 de Nexa POS. Hacé solo este módulo.

1. Leé completos, en este orden: antigravity/00-LEEME.md, la sección "Reglas comunes de la fase S" de antigravity/fase-S-salida.md y antigravity/modulos/M12-S3-negocio-sucursales.md.
2. Lo que no está escrito en este módulo está prohibido. No apliques nada a Supabase.
3. Trabajá en la rama feat/fase-s-salida de cada repositorio. Si el repositorio no aparece en la tabla del módulo, no lo toques.
4. Orden: migración → backend con sus pruebas → API y mock del frontend → pantallas.
5. Las validaciones van en el orden exacto en que están escritas, con los códigos de error y los textos indicados.
6. Encendé Docker Desktop y corré la verificación. Debe pasar entera.
7. Un commit por repositorio tocado, con el mensaje exacto del módulo.
8. Escribí la sección "M12" en antigravity/REPORTE-FASE-S.md (crealo si no existe) como dice "Entrega".
9. Detenete. No empieces M13. Escribime "M12 terminado".
```

## Especificación

### Problema (comprobado en el código)

- En `/configuracion/negocio`, el botón de "Datos Principales de la Empresa" solo muestra el aviso "Información de la empresa actualizada" (`settings-screen.tsx:610`). No guarda nada, porque el backend no tiene un endpoint para editar el negocio. Además, la tabla `tenants` no tiene política RLS de `update`.
- Los campos "Teléfono" y "Correo de facturación" no tienen columna en la base.
- No hay forma de crear una segunda sucursal: solo existe `GET /v1/branches`.
- `tenants_member_read` no revisa `m.active`: alguien con la membresía desactivada sigue viendo el nombre del negocio.

### Migración

```sql
drop policy tenants_member_read on public.tenants;
create policy tenants_member_read on public.tenants for select using (
  id in (select m.tenant_id from public.memberships m where m.user_id = auth.uid() and m.active)
);
create policy tenants_member_update on public.tenants for update
  using (id = public.current_tenant_id())
  with check (id = public.current_tenant_id());
```

### Backend

**`GET /v1/business`** (ya existe): agrega `iva_responsible` a la respuesta.

**`PUT /v1/business`** (`settings.rs`, permiso `settings:write`). Cuerpo: `{name, business_name, nit, dv, iva_responsible}`; los cuatro últimos pueden ser `null`. En este orden:
1. `name`: recortado, de 2 a 80 caracteres. Si no: 422 `BUSINESS_INVALID`, con `data: {field: "name"}`.
2. `business_name`: recortado; vacío pasa a `null`; más de 120 caracteres: 422 `BUSINESS_INVALID`, con `field: "business_name"`.
3. `nit`: solo dígitos; vacío pasa a `null`, y entonces `dv = null`. Con menos de 5 o más de 15 dígitos: 422 `BUSINESS_INVALID`, con `field: "nit"`. El DV se calcula con `procurement::nit_dv`. Si llegó `dv` y es distinto: 422 `NIT_DV_INVALID`.
4. Lee la fila actual con `select ... from tenants where id = $1 for update`. Si cambia `business_name`, `nit`, `dv` o `iva_responsible` y `scope.role != "DUENO"`: 403.
5. `update tenants set ...`. Auditoría `NEGOCIO_ACTUALIZADO`, con `before` y `after`.
6. Responde lo mismo que `GET /v1/business`.

**`POST /v1/branches`** (solo `DUENO`: si `scope.role != "DUENO"`, 403). Cuerpo `{name}`:
1. `tenancy::begin(pool, claims)` y luego `db::set_scope(&mut tx, scope.tenant, None).await?`. Sin eso, los triggers que crean la bodega y la caja de la sucursal nueva chocan con RLS cuando el JWT trae una sucursal.
2. `name` recortado, de 2 a 60 caracteres. Si no: 422 `BRANCH_INVALID`.
3. Si ya existe en el negocio una sucursal con el mismo nombre (comparando `lower(name)`): 409 `BRANCH_DUPLICATED`.
4. `insert into branches (tenant_id, name)`. Los triggers crean "Principal" y "Caja 1".
5. Auditoría `SUCURSAL_CREADA`. Respuesta 201: `{id, name}`.

**`PUT /v1/branches/{id}`** (permiso `settings:write`, con `tenancy::begin_in_branch(id)`). Cuerpo `{name}`, con las validaciones 2 y 3, sin contar la misma sucursal. Auditoría `SUCURSAL_ACTUALIZADA`. Respuesta `{id, name}`.

### Frontend (`settings-screen.tsx`, sección "Datos Principales de la Empresa")

- Campos: "Nombre comercial" (`name`), "Razón social" (`business_name`), "NIT" (`nit`), "DV" (solo lectura, calculado en el navegador con la función pura nueva `nitDv` de `lib/nit.ts`, con el mismo algoritmo que `procurement::nit_dv`) y "¿Es responsable de IVA?" (selector "Sí", "No" y "Sin definir", que valen `true`, `false` y `null`).
- **Quita** los campos "Teléfono" y "Correo de facturación".
- Si el usuario no es dueño, "Razón social", "NIT" y "¿Es responsable de IVA?" quedan deshabilitados, con el texto "Solo el dueño puede cambiar estos datos."
- El botón "Guardar datos" llama a `api.business.update(...)`, muestra "Datos del negocio guardados" y recarga. Los errores van con `friendlyError`.
- **Sucursales** (la sección que ya las lista en la misma pantalla): cada tarjeta agrega "Cambiar nombre" (diálogo con un campo y "Guardar"). Para el dueño, el botón "Nueva sucursal" abre un diálogo con "Nombre de la sucursal". Al crear, llama a `useSession().boot()` para que la sucursal aparezca en el selector.
- Mock en `lib/api/mock-platform.ts`, con las mismas validaciones.

Prueba unitaria del frontend no hay; la función `nitDv` debe dar `8` para `900123456` (el mismo caso de C7 de la fase 3).

### Pruebas (`s3_business`)

- **[P]** El dueño cambia el nombre, la razón social y el NIT `900123456` sin DV: queda guardado con DV `8` y hay auditoría `NEGOCIO_ACTUALIZADO`.
- **[P]** NIT `900123456` con `dv = 7`: 422 `NIT_DV_INVALID`.
- **[P]** `ADMIN_CAJA` cambia solo `name`: 200. `ADMIN_CAJA` cambia `nit`: 403.
- **[P]** El dueño crea la sucursal "Norte": existe, con su bodega "Principal" y su caja `CAJA-1`. Crear otra "norte": 409 `BRANCH_DUPLICATED`.
- **[P]** `ADMIN_CAJA` crea una sucursal: 403.
- **[P]** Con la membresía desactivada, `select name from tenants` devuelve 0 filas.

## Errores de este módulo

Agrégalos al backend (`error::coded`) y a `ERROR_MESSAGES` del frontend, con estos textos exactos:

| Código | HTTP | Texto en pantalla |
|---|---|---|
| `BUSINESS_INVALID` | 422 | Revisá los datos del negocio. |
| `BRANCH_INVALID` | 422 | El nombre de la sucursal debe tener entre 2 y 60 caracteres. |
| `BRANCH_DUPLICATED` | 409 | Ya existe una sucursal con ese nombre. |

`NIT_DV_INVALID` ya existe: no lo dupliques.

## Verificación

```sh
cd nexa-backend && ./scripts/test.sh "fmt --check && cargo clippy --all-targets -- -D warnings && cargo test"
cd ../nexa-frontend && npx tsc --noEmit && npm run lint
```

## Entrega

En `antigravity/REPORTE-FASE-S.md`, sección `## M12`:
1. Hash de cada commit y la lista de archivos tocados.
2. Las últimas 30 líneas de `test.sh`, y la salida de `tsc` y de `lint`.
3. La salida de los `grep` de la verificación, si los hay.
4. Una tabla con cada regla [P] del módulo y el nombre de su prueba. Las que falten, marcadas "FALTA".
5. Lo que no se pudo hacer, con el archivo y la línea. Escribe "no lo probé" antes que "funciona".
