-- Capa 4/2 - Movimientos de efectivo del turno, punto de reorden y traslados

-- ============================================================
-- Tipos maestros
-- ============================================================
insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id) values
  ('TIPOS DE MOVIMIENTO DE CAJA', 'CAJA_MOV', null);

insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.nombre, c.codigo, h.tima_id
from (values ('Ingreso de efectivo','INGRESO'),('Retiro de efectivo','RETIRO')) c(nombre, codigo)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'CAJA_MOV') h;

-- Traslados entre sucursales: salen de una y entran en otra con la misma referencia.
insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.nombre, c.codigo, h.tima_id
from (values ('Traslado (salida)','TRASLADO_SALIDA'),('Traslado (entrada)','TRASLADO_ENTRADA')) c(nombre, codigo)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'MOV') h;

-- ============================================================
-- Ingresos y retiros de efectivo durante el turno
-- ============================================================
-- Un retiro (pago de hielo, envio a banco) o un ingreso (cambio que trae el
-- dueno) mueve el efectivo esperado en gaveta sin ser una venta.
create table public.cash_movements (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  cash_session_id uuid not null,
  kind_id integer not null references public.tbl_tipos_maestra(tima_id),
  amount numeric(14,2) not null check (amount > 0),
  reason text not null check (length(trim(reason)) > 0),
  created_by uuid not null references public.users(id),
  created_at timestamptz not null default now(),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id),
  foreign key (tenant_id, cash_session_id) references public.cash_sessions (tenant_id, id)
);
create index cash_movements_session_idx on public.cash_movements (cash_session_id, created_at);

-- ============================================================
-- Punto de reorden por sucursal
-- ============================================================
alter table public.stock
  add column min_qty numeric(14,4) not null default 0 check (min_qty >= 0);

-- Referencia comun de las dos patas de un traslado.
create index inventory_movements_ref_idx on public.inventory_movements (ref_type, ref_id);

-- ============================================================
-- Row-Level Security y grants
-- ============================================================
alter table public.cash_movements enable row level security;

create policy cash_movements_isolation on public.cash_movements for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));

grant select, insert on public.cash_movements to authenticated;
