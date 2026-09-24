-- Capa 2 - Inventario y reservas

create type public.reservation_status as enum ('activa','confirmada','liberada','expirada');
create type public.movement_type as enum ('descuento','reposicion','ajuste');

create table public.stock (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  location_id uuid not null,
  product_id uuid not null,
  on_hand numeric(14,4) not null default 0,
  reserved numeric(14,4) not null default 0,
  updated_at timestamptz not null default now(),
  unique (location_id, product_id),
  check (on_hand >= 0),
  check (reserved >= 0),
  check (on_hand >= reserved),
  foreign key (tenant_id, location_id) references public.locations (tenant_id, id),
  foreign key (tenant_id, product_id) references public.products (tenant_id, id)
);

create table public.reservations (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  location_id uuid not null,
  order_id uuid,
  order_line_id uuid,
  product_id uuid not null,
  qty numeric(14,4) not null check (qty > 0),
  status public.reservation_status not null default 'activa',
  expires_at timestamptz,
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  foreign key (tenant_id, location_id) references public.locations (tenant_id, id),
  foreign key (tenant_id, product_id) references public.products (tenant_id, id),
  foreign key (tenant_id, order_id) references public.orders (tenant_id, id) on delete cascade,
  foreign key (tenant_id, order_line_id) references public.order_lines (tenant_id, id) on delete cascade
);
create index reservations_order_idx on public.reservations (order_id);
create index reservations_active_idx on public.reservations (tenant_id, location_id, product_id)
  where status = 'activa';

create table public.inventory_movements (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  location_id uuid not null,
  product_id uuid not null,
  type public.movement_type not null,
  qty numeric(14,4) not null check (qty > 0),
  cause text,
  ref_type text,
  ref_id uuid,
  created_by uuid references auth.users(id),
  created_at timestamptz not null default now(),
  foreign key (tenant_id, location_id) references public.locations (tenant_id, id),
  foreign key (tenant_id, product_id) references public.products (tenant_id, id)
);
create index inventory_movements_lookup on public.inventory_movements (tenant_id, location_id, product_id);

-- Row-Level Security
alter table public.stock enable row level security;
alter table public.reservations enable row level security;
alter table public.inventory_movements enable row level security;

create policy stock_isolation on public.stock for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());
create policy reservations_isolation on public.reservations for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());
create policy inventory_movements_isolation on public.inventory_movements for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

-- Grants
grant select, insert, update, delete on public.stock, public.reservations, public.inventory_movements to authenticated;
