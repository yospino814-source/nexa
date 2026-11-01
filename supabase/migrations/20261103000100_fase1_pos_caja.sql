-- Capa 1: POS y Caja (Fase 1)

-- ============================================================
-- F1.1 Cajas registradoras
-- ============================================================

create table public.cash_registers (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  name text not null check (length(trim(name)) between 1 and 60),
  code text not null check (code ~ '^[A-Z0-9-]{1,10}$'),
  default_opening numeric(14,2) not null default 0 check (default_opening >= 0),
  active boolean not null default true,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  unique (branch_id, code),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);

alter table public.cash_registers enable row level security;

create policy cash_registers_isolation on public.cash_registers for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));

grant select, insert, update, delete on public.cash_registers to authenticated;

-- Una caja "Caja 1" por cada sucursal existente
insert into public.cash_registers (tenant_id, branch_id, name, code)
select tenant_id, id, 'Caja 1', 'CAJA-1' from public.branches;

alter table public.cash_sessions add column register_id uuid;
update public.cash_sessions s set register_id = r.id
  from public.cash_registers r where r.branch_id = s.branch_id and r.code = 'CAJA-1';

-- Guardia: si hay dos turnos abiertos en la misma sucursal, la migración se detiene
do $$ begin
  if exists (select 1 from public.cash_sessions where closed_at is null
             group by branch_id having count(*) > 1) then
    raise exception 'Hay sucursales con mas de un turno abierto: cerrarlos antes de migrar';
  end if;
end $$;

alter table public.cash_sessions alter column register_id set not null;
alter table public.cash_sessions add constraint cash_sessions_register_fkey
  foreign key (tenant_id, register_id) references public.cash_registers (tenant_id, id);
create unique index cash_sessions_one_open_per_register
  on public.cash_sessions (register_id) where closed_at is null;

create or replace function public.branch_default_register() returns trigger language plpgsql as $$
begin
  insert into public.cash_registers (tenant_id, branch_id, name, code)
  values (new.tenant_id, new.id, 'Caja 1', 'CAJA-1');
  return new;
end $$;

drop trigger if exists branches_default_register on public.branches;
create trigger branches_default_register after insert on public.branches
  for each row execute function public.branch_default_register();


-- ============================================================
-- F1.2 Tipos de movimiento de caja y esperado
-- ============================================================

insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.nombre, c.codigo, h.tima_id
from (values ('Abono de cartera','ABONO_CARTERA'), ('Abono de separado','ABONO_SEPARADO'),
             ('Recarga de saldo','RECARGA_SALDO'), ('Devolucion','DEVOLUCION'),
             ('Propina pagada','PROPINA_PAGADA'), ('Pago a proveedor','PAGO_PROVEEDOR'),
             ('Gasto','GASTO')) c(nombre, codigo)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'CAJA_MOV') h
on conflict (padre_id, tima_codigo) do nothing;

alter table public.cash_movements
  add column reason_code text check (reason_code in ('CONSIGNACION','GASTO_MENOR','PRESTAMO','OTRO')),
  add column ref_type text,
  add column ref_id uuid;


-- ============================================================
-- F1.4 Clientes mínimos y cliente en la venta
-- ============================================================

create table public.customers (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  doc_type text check (doc_type in ('CC','NIT','CE','PP','TI','PEP','PPT','NIUP')),
  doc_number text check (doc_number ~ '^[A-Z0-9]{3,20}$'),
  dv char(1) check (dv ~ '^[0-9]$'),
  name text not null check (length(trim(name)) between 1 and 120),
  phone text check (phone ~ '^[0-9]{7,15}$'),
  email text,
  price_tier_id uuid,
  archived_at timestamptz,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  check ((doc_type is null) = (doc_number is null)),
  check (dv is null or doc_type = 'NIT')
);
create unique index customers_doc_idx on public.customers (tenant_id, doc_type, doc_number)
  where doc_number is not null and archived_at is null;
create index customers_name_idx on public.customers (tenant_id, lower(name));

alter table public.customers enable row level security;
create policy customers_isolation on public.customers
  for all to authenticated
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());
grant select, insert, update on public.customers to authenticated;

alter table public.orders
  add column customer_id uuid,
  add column customer_alias text check (customer_alias is null or length(customer_alias) between 1 and 40),
  add constraint orders_customer_fkey foreign key (tenant_id, customer_id) references public.customers (tenant_id, id),
  add constraint orders_customer_xor check (customer_id is null or customer_alias is null);
