-- Capa 3 - Venta (orden generica, lineas y mesas)

create type public.order_channel as enum ('dine_in','qr_self_service','whatsapp','pos_counter','delivery');
create type public.order_status as enum ('creada','en_preparacion','lista','entregada','cobrada','cerrada','cancelada');
create type public.table_status as enum ('libre','ocupada');

create table public.tables (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  location_id uuid not null,
  code text not null,
  qr_token text not null unique default replace(gen_random_uuid()::text, '-', ''),
  status public.table_status not null default 'libre',
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  unique (tenant_id, location_id, code),
  foreign key (tenant_id, location_id) references public.locations (tenant_id, id)
);

create table public.orders (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  location_id uuid not null,
  table_id uuid,
  channel public.order_channel not null,
  status public.order_status not null default 'creada',
  responsible_user_id uuid not null references auth.users(id),
  notes text,
  created_at timestamptz not null default now(),
  closed_at timestamptz,
  cancelled_at timestamptz,
  unique (tenant_id, id),
  foreign key (tenant_id, location_id) references public.locations (tenant_id, id),
  foreign key (tenant_id, table_id) references public.tables (tenant_id, id)
);
create index orders_tenant_location_idx on public.orders (tenant_id, location_id);
create index orders_table_idx on public.orders (table_id);
create unique index orders_one_open_per_table on public.orders (table_id)
  where table_id is not null and status in ('creada','en_preparacion','lista','entregada');

create table public.order_lines (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  order_id uuid not null,
  product_id uuid not null,
  qty numeric(14,4) not null check (qty > 0),
  unit_price numeric(12,2) not null check (unit_price >= 0),
  line_total numeric(14,2) generated always as (qty * unit_price) stored,
  notes text,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  foreign key (tenant_id, order_id) references public.orders (tenant_id, id) on delete cascade,
  foreign key (tenant_id, product_id) references public.products (tenant_id, id)
);
create index order_lines_order_idx on public.order_lines (order_id);

-- Row-Level Security
alter table public.tables enable row level security;
alter table public.orders enable row level security;
alter table public.order_lines enable row level security;

create policy tables_isolation on public.tables for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());
create policy orders_isolation on public.orders for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());
create policy order_lines_isolation on public.order_lines for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

-- Grants
grant select, insert, update, delete on public.tables, public.orders, public.order_lines to authenticated;
