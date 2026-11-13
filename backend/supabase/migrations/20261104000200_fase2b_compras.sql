-- Migración única para Fase 2B: Compras, Proveedores, CxP, Gastos, Lotes y Ajustes
-- 20261104000200_fase2b_compras.sql

-- F2.11 Proveedores y productos por proveedor
alter table public.suppliers
  add column if not exists doc_type text not null default 'NIT' check (doc_type in ('NIT','CC','CE')),
  add column if not exists dv char(1) check (dv ~ '^[0-9]$'),
  add column if not exists person_type text not null default 'JURIDICA' check (person_type in ('NATURAL','JURIDICA')),
  add column if not exists iva_responsible boolean not null default true,
  add column if not exists self_withholding boolean not null default false,
  add column if not exists payment_terms text not null default 'CONTADO' check (payment_terms in ('CONTADO','CREDITO')),
  add column if not exists credit_days integer not null default 0 check (credit_days between 0 and 180),
  add column if not exists delivery_days integer check (delivery_days between 0 and 60),
  add column if not exists category text not null default 'MERCANCIA' check (category in ('MERCANCIA','TRANSPORTE','SERVICIOS')),
  add column if not exists bank_name text,
  add column if not exists bank_account_type text check (bank_account_type in ('AHORROS','CORRIENTE')),
  add column if not exists bank_account_number text check (bank_account_number ~ '^[0-9]{6,20}$'),
  add column if not exists archived_at timestamptz;

do $$
begin
  if not exists (select 1 from pg_constraint where conname = 'suppliers_credit') then
    alter table public.suppliers add constraint suppliers_credit check (payment_terms = 'CREDITO' or credit_days = 0);
  end if;
end $$;

create table if not exists public.supplier_products (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  supplier_id uuid not null,
  product_id uuid not null,
  supplier_code text,
  last_cost numeric(14,4),
  min_order_qty numeric(14,4) check (min_order_qty > 0),
  preferred boolean not null default false,
  primary key (supplier_id, product_id),
  foreign key (tenant_id, supplier_id) references public.suppliers (tenant_id, id) on delete cascade,
  foreign key (tenant_id, product_id) references public.products (tenant_id, id) on delete cascade
);

create unique index if not exists supplier_products_one_preferred
  on public.supplier_products (product_id) where preferred;

alter table public.tenants add column if not exists iva_responsible boolean;

alter table public.supplier_products enable row level security;
drop policy if exists supplier_products_isolation on public.supplier_products;
create policy supplier_products_isolation on public.supplier_products for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());
grant select, insert, update, delete on public.supplier_products to authenticated;

-- F2.12 Compras v2 (purchase_receipts como orden de compra)
update public.tbl_tipos_maestra set tima_nombre = 'Borrador'
 where tima_codigo = 'PENDIENTE' and padre_id = (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'PUR');

insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.n, c.c, h.tima_id from (values ('Aprobada','APROBADA'),('Parcial','PARCIAL')) c(n, c)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'PUR') h
on conflict (padre_id, tima_codigo) do nothing;

alter table public.purchase_receipts
  add column if not exists location_id uuid,
  add column if not exists expected_at date,
  add column if not exists discount_amount numeric(14,2) not null default 0 check (discount_amount >= 0),
  add column if not exists freight_amount numeric(14,2) not null default 0 check (freight_amount >= 0),
  add column if not exists freight_mode text not null default 'PRORRATEAR' check (freight_mode in ('PRORRATEAR','GASTO')),
  add column if not exists payment_terms text not null default 'CONTADO' check (payment_terms in ('CONTADO','CREDITO')),
  add column if not exists credit_days integer not null default 0,
  add column if not exists iva_to_cost boolean not null default false,
  add column if not exists approved_by uuid references public.users(id),
  add column if not exists approved_at timestamptz;

-- Asignar location_id por defecto a compras existentes que no lo tengan
update public.purchase_receipts p
   set location_id = l.id
  from public.locations l
 where l.branch_id = p.branch_id and l.is_default and p.location_id is null;

update public.purchase_receipts p
   set location_id = l.id
  from (select distinct on (branch_id) id, branch_id from public.locations) l
 where l.branch_id = p.branch_id and p.location_id is null;

-- Si alguna orden aún no tiene location_id (en caso de que no haya locations para esa sucursal),
-- insertamos la bodega por defecto para esa sucursal
insert into public.locations (tenant_id, branch_id, name, is_default)
select distinct p.tenant_id, p.branch_id, 'Principal', true
  from public.purchase_receipts p
 where p.location_id is null
on conflict do nothing;

update public.purchase_receipts p
   set location_id = l.id
  from public.locations l
 where l.branch_id = p.branch_id and p.location_id is null;

alter table public.purchase_receipts alter column location_id set not null;

do $$
begin
  if not exists (select 1 from pg_constraint where conname = 'purchase_receipts_location_fkey') then
    alter table public.purchase_receipts
      add constraint purchase_receipts_location_fkey
      foreign key (tenant_id, branch_id, location_id)
      references public.locations (tenant_id, branch_id, id);
  end if;
end $$;

alter table public.purchase_receipt_items
  add column if not exists qty_received numeric(14,4) not null default 0 check (qty_received >= 0),
  add column if not exists discount_alloc numeric(14,2) not null default 0,
  add column if not exists freight_alloc numeric(14,2) not null default 0,
  add column if not exists tax_amount numeric(14,2) not null default 0,
  add column if not exists landed_unit_cost numeric(14,4),
  add column if not exists new_sale_price numeric(14,2) check (new_sale_price >= 0);

do $$
begin
  if not exists (select 1 from pg_constraint where conname = 'pri_received_le_qty') then
    alter table public.purchase_receipt_items
      add constraint pri_received_le_qty check (qty_received <= qty);
  end if;
end $$;

create table if not exists public.purchase_receptions (
  id uuid primary key,
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  purchase_id uuid not null,
  received_by uuid not null references public.users(id),
  received_at timestamptz not null default now(),
  client_request jsonb not null,
  unique (tenant_id, id),
  foreign key (tenant_id, purchase_id) references public.purchase_receipts (tenant_id, id)
);

alter table public.purchase_receptions enable row level security;
drop policy if exists purchase_receptions_isolation on public.purchase_receptions;
create policy purchase_receptions_isolation on public.purchase_receptions for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));
grant select, insert, update, delete on public.purchase_receptions to authenticated;

create table if not exists public.purchase_reception_items (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  reception_id uuid not null,
  purchase_item_id uuid not null,
  qty numeric(14,4) not null check (qty > 0),
  lot_id uuid,
  foreign key (tenant_id, reception_id) references public.purchase_receptions (tenant_id, id) on delete cascade,
  foreign key (tenant_id, purchase_item_id) references public.purchase_receipt_items (tenant_id, id)
);

alter table public.purchase_reception_items enable row level security;
drop policy if exists purchase_reception_items_isolation on public.purchase_reception_items;
create policy purchase_reception_items_isolation on public.purchase_reception_items for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());
grant select, insert, update, delete on public.purchase_reception_items to authenticated;

-- F2.13 Cuentas por pagar
create table if not exists public.payables (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  supplier_id uuid,
  source_type text not null check (source_type in ('COMPRA','GASTO')),
  source_id uuid not null,
  amount_total numeric(14,2) not null check (amount_total >= 0),
  amount_paid numeric(14,2) not null default 0 check (amount_paid >= 0),
  status text not null default 'ABIERTA' check (status in ('ABIERTA','PARCIAL','PAGADA','CASTIGADA')),
  due_date date not null,
  written_off_reason text,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  unique (source_type, source_id),
  check (amount_paid <= amount_total),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id),
  foreign key (tenant_id, supplier_id) references public.suppliers (tenant_id, id)
);

alter table public.payables enable row level security;
drop policy if exists payables_isolation on public.payables;
create policy payables_isolation on public.payables for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));
grant select, insert, update, delete on public.payables to authenticated;

create table if not exists public.payable_payments (
  id uuid primary key,
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  payable_id uuid not null,
  amount numeric(14,2) not null check (amount > 0),
  method text not null check (method in ('EFECTIVO','TRANSFERENCIA','CHEQUE')),
  reference text,
  cash_session_id uuid,
  paid_at timestamptz not null default now(),
  created_by uuid not null references public.users(id),
  client_request jsonb not null,
  foreign key (tenant_id, payable_id) references public.payables (tenant_id, id),
  foreign key (tenant_id, cash_session_id) references public.cash_sessions (tenant_id, id)
);

alter table public.payable_payments enable row level security;
drop policy if exists payable_payments_isolation on public.payable_payments;
create policy payable_payments_isolation on public.payable_payments for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());
grant select, insert, update, delete on public.payable_payments to authenticated;

-- F2.14 Gastos
create table if not exists public.expense_categories (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  name text not null check (length(trim(name)) between 1 and 40),
  color text not null default '#64748B' check (color ~ '^#[0-9A-Fa-f]{6}$'),
  active boolean not null default true,
  unique (tenant_id, id),
  unique (tenant_id, name)
);

alter table public.expense_categories enable row level security;
drop policy if exists expense_categories_isolation on public.expense_categories;
create policy expense_categories_isolation on public.expense_categories for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());
grant select, insert, update, delete on public.expense_categories to authenticated;

create table if not exists public.expenses (
  id uuid primary key,
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  prefix text not null,
  number bigint not null,
  category_id uuid not null,
  supplier_id uuid,
  description text not null check (length(trim(description)) between 3 and 200),
  amount numeric(14,2) not null check (amount > 0),
  tax_amount numeric(14,2) not null default 0 check (tax_amount >= 0),
  total numeric(14,2) generated always as (amount + tax_amount) stored,
  expense_date date not null,
  status text not null default 'PENDIENTE' check (status in ('PENDIENTE','APROBADO','RECHAZADO','CANCELADO','PAGADO')),
  created_by uuid not null references public.users(id),
  decided_by uuid references public.users(id),
  decided_at timestamptz,
  reject_reason text,
  client_request jsonb not null,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  unique (branch_id, prefix, number),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id),
  foreign key (tenant_id, category_id) references public.expense_categories (tenant_id, id),
  foreign key (tenant_id, supplier_id) references public.suppliers (tenant_id, id)
);

alter table public.expenses enable row level security;
drop policy if exists expenses_isolation on public.expenses;
create policy expenses_isolation on public.expenses for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));
grant select, insert, update, delete on public.expenses to authenticated;

insert into public.permissions (code, module, description) values
  ('expenses:create','expenses','Registrar gastos'),
  ('expenses:approve','expenses','Aprobar gastos')
on conflict (code) do nothing;

insert into public.role_permissions (role_id, permission_code)
select r.tima_id, p.code
from public.tbl_tipos_maestra r
cross join (values ('expenses:create'), ('expenses:approve')) as p(code)
where r.padre_id = (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
  and r.tima_codigo in ('DUENO', 'ADMIN_CAJA')
on conflict do nothing;

insert into public.role_permissions (role_id, permission_code)
select r.tima_id, 'expenses:create'
from public.tbl_tipos_maestra r
where r.padre_id = (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
  and r.tima_codigo = 'SUPERVISOR'
on conflict do nothing;

insert into public.expense_categories (tenant_id, name, color)
select t.id, c.name, c.color
  from public.tenants t
 cross join (values
   ('Arriendo', '#64748B'),
   ('Servicios públicos', '#0EA5E9'),
   ('Nómina', '#10B981'),
   ('Fletes', '#F59E0B'),
   ('Mantenimiento', '#8B5CF6'),
   ('Otros', '#94A3B8')
 ) as c(name, color)
on conflict (tenant_id, name) do nothing;

create or replace function public.tenant_default_expense_categories() returns trigger language plpgsql as $$
begin
  insert into public.expense_categories (tenant_id, name, color)
  values
    (new.id, 'Arriendo', '#64748B'),
    (new.id, 'Servicios públicos', '#0EA5E9'),
    (new.id, 'Nómina', '#10B981'),
    (new.id, 'Fletes', '#F59E0B'),
    (new.id, 'Mantenimiento', '#8B5CF6'),
    (new.id, 'Otros', '#94A3B8')
  on conflict (tenant_id, name) do nothing;
  return new;
end $$;

drop trigger if exists tenants_default_expense_categories on public.tenants;
create trigger tenants_default_expense_categories after insert on public.tenants
  for each row execute function public.tenant_default_expense_categories();

-- F2.15 Lotes y vencimientos
create table if not exists public.lots (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  location_id uuid not null,
  product_id uuid not null,
  code text not null check (length(trim(code)) between 1 and 30),
  expires_at date not null,
  qty numeric(14,4) not null default 0,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  unique (location_id, product_id, code),
  foreign key (tenant_id, branch_id, location_id) references public.locations (tenant_id, branch_id, id),
  foreign key (tenant_id, product_id) references public.products (tenant_id, id)
);

alter table public.lots enable row level security;
drop policy if exists lots_isolation on public.lots;
create policy lots_isolation on public.lots for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));
grant select, insert, update, delete on public.lots to authenticated;

create table if not exists public.order_item_lots (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  order_item_id uuid not null,
  lot_id uuid not null,
  qty numeric(14,4) not null check (qty > 0),
  primary key (order_item_id, lot_id),
  foreign key (tenant_id, order_item_id) references public.order_items (tenant_id, id) on delete cascade,
  foreign key (tenant_id, lot_id) references public.lots (tenant_id, id)
);

alter table public.order_item_lots enable row level security;
drop policy if exists order_item_lots_isolation on public.order_item_lots;
create policy order_item_lots_isolation on public.order_item_lots for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());
grant select, insert, update, delete on public.order_item_lots to authenticated;

-- F2.16 Ajustes de inventario con aprobación
create table if not exists public.inventory_adjustments (
  id uuid primary key,
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  location_id uuid not null,
  product_id uuid not null,
  lot_id uuid,
  qty_delta numeric(14,4) not null check (qty_delta <> 0),
  reason_code text not null check (reason_code in ('DANO','PERDIDA','ROBO','VENCIMIENTO','CONTEO','CORRECCION','CARGA_INICIAL')),
  notes text check (length(notes) <= 300),
  unit_cost numeric(14,4) not null,
  value numeric(14,2) not null,
  status text not null check (status in ('PENDIENTE','APLICADO','RECHAZADO')),
  requested_by uuid not null references public.users(id),
  decided_by uuid references public.users(id),
  decided_at timestamptz,
  reject_reason text,
  client_request jsonb not null,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  foreign key (tenant_id, branch_id, location_id) references public.locations (tenant_id, branch_id, id),
  foreign key (tenant_id, product_id) references public.products (tenant_id, id)
);

alter table public.inventory_adjustments enable row level security;
drop policy if exists inventory_adjustments_isolation on public.inventory_adjustments;
create policy inventory_adjustments_isolation on public.inventory_adjustments for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));
grant select, insert, update, delete on public.inventory_adjustments to authenticated;
