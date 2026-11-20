# M02 Migración de la fase 3 corregida

| Dato | Valor |
|---|---|
| Fase | 3 |
| Repositorio | Solo `nexa-backend` |
| Rama | `feat/fase-3-clientes` |
| Archivo | `supabase/migrations/20261106000100_fase3_clientes.sql` (ya existe; **no está aplicado**, así que se corrige en el mismo archivo) |
| Commit | `feat(fase3): correcciones de migracion` |
| Reporte | Sección "M02" de `antigravity/REPORTE-FASE-3.md` |

## Prompt para Antigravity

```text
Vas a hacer el módulo M02 de Nexa POS. Hacé solo este módulo.

1. Leé completos, en este orden: antigravity/00-LEEME.md y antigravity/modulos/M02-fase3-migracion.md.
2. Trabajá solo en nexa-backend, en la rama feat/fase-3-clientes.
3. En esa rama hay cambios sin commit de otro módulo (credit.rs, sales.rs, refunds.rs, routes.rs, customers.rs, main.rs y db_tests.rs). No los toques, no los borres y no los incluyas en este commit: hacé git add solo del archivo de migración.
4. Cambiá el archivo de migración exactamente como dice "Qué hacer". No toques otras migraciones. No apliques nada a Supabase.
5. Encendé Docker Desktop y corré la verificación. Si la salida dice "failed to connect to the docker API", eso no es un resultado: anotalo y no hagas el commit.
6. Commit con el mensaje exacto del módulo.
7. Escribí la sección "M02" en antigravity/REPORTE-FASE-3.md como dice "Entrega".
8. Detenete. No empieces M03. Escribime "M02 terminado".
```

## Qué hacer

Seis cambios, en este orden.

### 1. Permisos: `do nothing` (línea 8)

Reemplaza:

```sql
on conflict (code) do update set description = excluded.description, module = excluded.module;
```

por:

```sql
on conflict (code) do nothing;
```

### 2. `SUPERVISOR` recibe también `credit:sell`

Después del bloque que empieza con `-- SUPERVISOR recibe credit:override` y termina en `on conflict do nothing;`, agrega:

```sql
-- SUPERVISOR recibe credit:sell
insert into public.role_permissions (role_id, permission_code)
select r.tima_id, p.code
from public.tbl_tipos_maestra r
cross join public.permissions p
where r.padre_id = (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
  and r.tima_codigo = 'SUPERVISOR'
  and p.code = 'credit:sell'
on conflict do nothing;
```

### 3. `receivables.principal` admite 0 (línea 76)

```sql
  principal numeric(14,2) not null check (principal >= 0),
```

Motivo: devolver completa una venta a crédito deja `principal = 0`. Con `> 0`, la devolución revienta con error 500.

### 4. Función de posición de crédito de todo el negocio

Agrégala justo **después** de la creación de `receivable_installments` y su RLS, y **antes** de `-- 4. F3.3 Abonos y cuotas`. Cópiala tal cual:

```sql
create or replace function public.customer_credit_position(p_customer uuid)
returns table (balance numeric(14,2), max_days_overdue integer)
language sql
stable
security definer
set search_path = ''
as $$
  with x as (
    select r.amount_total - r.amount_paid as saldo,
           greatest(0, (now() at time zone b.timezone)::date - coalesce(i.first_due, r.due_date)) as dias
      from public.receivables r
      join public.branches b on b.id = r.branch_id
      left join lateral (
        select min(ri.due_date) as first_due
          from public.receivable_installments ri
         where ri.receivable_id = r.id and ri.amount_paid < ri.amount
      ) i on true
     where r.tenant_id = public.current_tenant_id()
       and r.customer_id = p_customer
       and r.status in ('ABIERTA','PARCIAL')
  )
  select coalesce(sum(saldo), 0)::numeric(14,2), coalesce(max(dias), 0)::integer from x
$$;
revoke all on function public.customer_credit_position(uuid) from public, anon;
grant execute on function public.customer_credit_position(uuid) to authenticated;
```

Motivo: `receivables` tiene RLS por sucursal, pero el cupo es de todo el negocio. Sin esta función, un cajero de la sucursal B no ve la deuda que el cliente tiene en la sucursal A.

### 5. Columna de devoluciones aplicadas a la cuenta

Agrégala justo **después** del bloque `do $$ ... refunds_method_check ... $$;`:

```sql
alter table public.refunds
  add column if not exists receivable_applied numeric(14,2) not null default 0 check (receivable_applied >= 0);
```

### 6. Índices

Agrégalos justo **después** de la creación de `receivable_payments` y su RLS:

```sql
create index if not exists receivables_customer_idx on public.receivables (tenant_id, customer_id) where status in ('ABIERTA','PARCIAL');
create index if not exists receivable_payments_receivable_idx on public.receivable_payments (receivable_id, created_at);
```

## Lo que no se cambia

Las tablas de F3.4, F3.6, F3.7 y F3.8 (pospuestas) se quedan en la migración tal como están, sin código.

## Verificación

```sh
cd nexa-backend
grep -n "customer_credit_position\|receivable_applied\|principal >= 0\|receivables_customer_idx\|receivable_payments_receivable_idx\|do update" supabase/migrations/20261106000100_fase3_clientes.sql
./scripts/test.sh "--version"
```

- El `grep` debe mostrar las cinco primeras cadenas y **ninguna** línea con `do update`.
- `./scripts/test.sh "--version"` levanta Postgres, aplica **todas** las migraciones y solo después corre `cargo --version`, sin compilar. Así se prueba la migración aunque la rama todavía no compile (eso se arregla en M03).
  - Si una migración falla, el script imprime "La base de prueba no arranco: una migracion fallo." con el error de SQL. Corrige y repite.
  - Si imprime la versión de cargo (`cargo 1.x.x`), la migración pasó.
- **No** corras `cargo test` en este módulo.

## Entrega

En `antigravity/REPORTE-FASE-3.md`, sección `## M02`:
1. Hash del commit.
2. Salida del `grep`.
3. Salida completa de `./scripts/test.sh "--version"`.
