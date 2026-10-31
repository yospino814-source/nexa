-- Capa 3 - Proveedores e ingresos de compra
--
-- Traida al repositorio desde Supabase (proyecto elbshjaaoxivgndkfjvx), donde
-- ya estaba aplicada con esta misma version. Contenido identico al historial
-- de supabase_migrations.schema_migrations, salvo el indice uq_padre_codigo de
-- abajo, que en Supabase se habia creado a mano y del que dependen los
-- `on conflict (padre_id, tima_codigo)` de esta migracion.
--
-- Pendiente (pantalla 08 Compras): recibir una compra todavia no suma stock
-- ni registra el movimiento en el kardex ni actualiza el costo; los
-- proveedores quedan atados a una sucursal; falta el estado de revision.

-- Unicidad de codigo por cabecera en la tabla maestra (ya existe en Supabase).
create unique index if not exists uq_padre_codigo
  on public.tbl_tipos_maestra (padre_id, tima_codigo);

create table public.suppliers (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  name text not null,
  tax_id text,
  contact_name text,
  phone text,
  email text,
  address text,
  notes text,
  active boolean not null default true,
  created_at timestamptz not null default now(),
  unique (tenant_id, branch_id, id),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);
create index suppliers_branch_idx on public.suppliers (branch_id);

create table public.purchase_receipts (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  supplier_id uuid not null,
  status_id integer not null references public.tbl_tipos_maestra(tima_id),
  document_number text,
  subtotal numeric(14,2) not null default 0,
  tax_amount numeric(14,2) not null default 0,
  total_amount numeric(14,2) not null default 0,
  received_at timestamptz,
  notes text,
  created_by uuid references auth.users(id),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (tenant_id, id),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id),
  foreign key (tenant_id, branch_id, supplier_id) references public.suppliers (tenant_id, branch_id, id)
);
create index purchase_receipts_branch_idx on public.purchase_receipts (branch_id);
create unique index purchase_receipts_document_idx
  on public.purchase_receipts (tenant_id, branch_id, supplier_id, document_number)
  where document_number is not null;

create table public.purchase_receipt_items (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  purchase_receipt_id uuid not null,
  product_id uuid not null,
  qty numeric(14,4) not null check (qty > 0),
  unit_cost numeric(12,2) not null check (unit_cost >= 0),
  line_total numeric(14,2) generated always as (qty * unit_cost) stored,
  tax_type_id integer references public.tbl_tipos_maestra(tima_id),
  notes text,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id),
  foreign key (tenant_id, purchase_receipt_id) references public.purchase_receipts (tenant_id, id) on delete cascade,
  foreign key (tenant_id, product_id) references public.products (tenant_id, id)
);
create index purchase_receipt_items_receipt_idx on public.purchase_receipt_items (purchase_receipt_id);

-- Row-Level Security
alter table public.suppliers enable row level security;
alter table public.purchase_receipts enable row level security;
alter table public.purchase_receipt_items enable row level security;

create policy suppliers_isolation on public.suppliers for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));

create policy purchase_receipts_isolation on public.purchase_receipts for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));

create policy purchase_receipt_items_isolation on public.purchase_receipt_items for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));

-- Grants
grant select, insert, update, delete on public.suppliers, public.purchase_receipts, public.purchase_receipt_items to authenticated;

-- ============================================================
-- Tipo maestro de estado de compra (idempotente)
-- ============================================================
insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
values ('TIPOS DE ESTADO DE COMPRA', 'PUR', null)
on conflict (tima_codigo) where padre_id is null do nothing;

insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.nombre, c.codigo, h.tima_id
from (values ('Recibida','RECIBIDA'),('Anulada','ANULADA')) c(nombre, codigo)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'PUR') h
on conflict (padre_id, tima_codigo) do nothing;
