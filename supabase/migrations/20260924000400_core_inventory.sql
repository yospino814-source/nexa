-- Capa 2 - Inventario y reservas

create table public.stock (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  product_id uuid not null,
  on_hand numeric(14,4) not null default 0,
  reserved numeric(14,4) not null default 0,
  updated_at timestamptz not null default now(),
  unique (branch_id, product_id),
  check (on_hand >= 0),
  check (reserved >= 0),
  check (on_hand >= reserved),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id),
  foreign key (tenant_id, product_id) references public.products (tenant_id, id)
);

create table public.stock_reservations (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  order_id uuid,
  order_item_id uuid,
  product_id uuid not null,
  qty numeric(14,4) not null check (qty > 0),
  status_id integer not null references public.tbl_tipos_maestra(tima_id),
  expires_at timestamptz,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id),
  foreign key (tenant_id, product_id) references public.products (tenant_id, id),
  foreign key (tenant_id, order_id) references public.orders (tenant_id, id) on delete cascade,
  foreign key (tenant_id, order_item_id) references public.order_items (tenant_id, id) on delete cascade
);
create index stock_reservations_order_idx on public.stock_reservations (order_id);
create index stock_reservations_lookup_idx on public.stock_reservations (tenant_id, branch_id, product_id, status_id);

create table public.inventory_movements (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  product_id uuid not null,
  movement_type_id integer not null references public.tbl_tipos_maestra(tima_id),
  qty numeric(14,4) not null check (qty > 0),
  cause text,
  ref_type text,
  ref_id uuid,
  created_by uuid references auth.users(id),
  created_at timestamptz not null default now(),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id),
  foreign key (tenant_id, product_id) references public.products (tenant_id, id)
);
create index inventory_movements_lookup on public.inventory_movements (tenant_id, branch_id, product_id);

-- Row-Level Security
alter table public.stock enable row level security;
alter table public.stock_reservations enable row level security;
alter table public.inventory_movements enable row level security;

create policy stock_isolation on public.stock for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));

create policy stock_reservations_isolation on public.stock_reservations for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));

create policy inventory_movements_isolation on public.inventory_movements for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));

-- Grants
grant select, insert, update, delete on public.stock, public.stock_reservations, public.inventory_movements to authenticated;
