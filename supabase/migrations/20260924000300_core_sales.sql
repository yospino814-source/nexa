-- Capa 3 - Venta (orden generica, lineas y mesas)

create table public.kds_stations (
  id integer generated always as identity primary key,
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  name text not null,
  active boolean not null default true,
  created_at timestamptz not null default now(),
  unique (branch_id, name),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);

create table public.restaurant_tables (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  code text not null,
  qr_token text not null unique default replace(gen_random_uuid()::text, '-', ''),
  status_id integer not null references public.tbl_tipos_maestra(tima_id),
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  unique (tenant_id, branch_id, code),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);
create index restaurant_tables_branch_idx on public.restaurant_tables (tenant_id, branch_id);

create table public.orders (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  table_id uuid,
  sale_type_id integer not null references public.tbl_tipos_maestra(tima_id),
  status_id integer not null references public.tbl_tipos_maestra(tima_id),
  waiter_id uuid references public.users(id),
  notes text,
  subtotal numeric(14,2) not null default 0,
  inc_amount numeric(14,2) not null default 0,
  tip_amount numeric(14,2) not null default 0,
  total_amount numeric(14,2) not null default 0,
  created_at timestamptz not null default now(),
  closed_at timestamptz,
  cancelled_at timestamptz,
  unique (tenant_id, id),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id),
  foreign key (tenant_id, table_id) references public.restaurant_tables (tenant_id, id)
);
create index orders_tenant_branch_idx on public.orders (tenant_id, branch_id);
create index orders_table_idx on public.orders (table_id);

create table public.order_items (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  order_id uuid not null,
  product_id uuid not null,
  qty numeric(14,4) not null check (qty > 0),
  unit_price numeric(12,2) not null check (unit_price >= 0),
  line_total numeric(14,2) generated always as (qty * unit_price) stored,
  station_id integer references public.kds_stations(id),
  status_id integer references public.tbl_tipos_maestra(tima_id),
  notes text,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  foreign key (tenant_id, order_id) references public.orders (tenant_id, id) on delete cascade,
  foreign key (tenant_id, product_id) references public.products (tenant_id, id),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);
create index order_items_order_idx on public.order_items (order_id);

-- Row-Level Security
alter table public.kds_stations enable row level security;
alter table public.restaurant_tables enable row level security;
alter table public.orders enable row level security;
alter table public.order_items enable row level security;

create policy kds_stations_isolation on public.kds_stations for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));

create policy restaurant_tables_isolation on public.restaurant_tables for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));

create policy orders_isolation on public.orders for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));

create policy order_items_isolation on public.order_items for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));

-- Grants
grant select, insert, update, delete on public.kds_stations, public.restaurant_tables, public.orders, public.order_items to authenticated;
