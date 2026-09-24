-- Capa 1 - Catalogo, precios e impuestos

create type public.product_type as enum ('sellable','supply','both');
create type public.station as enum ('cocina','barra');
create type public.uom as enum ('und','g','ml');

create table public.taxes (
  id uuid primary key default gen_random_uuid(),
  code text not null unique,
  name text not null,
  rate numeric(5,4) not null check (rate >= 0 and rate <= 1),
  active boolean not null default true
);

create table public.categories (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  name text not null,
  sort_order integer not null default 0,
  active boolean not null default true,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  unique (tenant_id, name)
);

create table public.products (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  category_id uuid,
  sku text,
  name text not null,
  product_type public.product_type not null default 'sellable',
  station public.station,
  tax_id uuid references public.taxes(id),
  uom public.uom not null default 'und',
  tracks_inventory boolean not null default true,
  active boolean not null default true,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  unique (tenant_id, sku),
  foreign key (tenant_id, category_id) references public.categories (tenant_id, id)
);
create index products_tenant_idx on public.products (tenant_id);

create table public.product_prices (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  product_id uuid not null,
  location_id uuid,
  price numeric(12,2) not null check (price >= 0),
  currency char(3) not null default 'COP',
  created_at timestamptz not null default now(),
  unique nulls not distinct (product_id, location_id),
  foreign key (tenant_id, product_id) references public.products (tenant_id, id) on delete cascade,
  foreign key (tenant_id, location_id) references public.locations (tenant_id, id)
);
create index product_prices_lookup on public.product_prices (product_id, location_id);

create table public.recipes (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  product_id uuid not null,
  yield_qty numeric(12,4) not null default 1 check (yield_qty > 0),
  unique (product_id),
  unique (tenant_id, id),
  foreign key (tenant_id, product_id) references public.products (tenant_id, id) on delete cascade
);

create table public.recipe_items (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  recipe_id uuid not null,
  input_product_id uuid not null,
  qty numeric(12,4) not null check (qty > 0),
  unique (recipe_id, input_product_id),
  foreign key (tenant_id, recipe_id) references public.recipes (tenant_id, id) on delete cascade,
  foreign key (tenant_id, input_product_id) references public.products (tenant_id, id)
);

-- Row-Level Security
alter table public.taxes enable row level security;
alter table public.categories enable row level security;
alter table public.products enable row level security;
alter table public.product_prices enable row level security;
alter table public.recipes enable row level security;
alter table public.recipe_items enable row level security;

create policy taxes_read on public.taxes for select using (true);

create policy categories_isolation on public.categories for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());
create policy products_isolation on public.products for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());
create policy product_prices_isolation on public.product_prices for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());
create policy recipes_isolation on public.recipes for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());
create policy recipe_items_isolation on public.recipe_items for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

-- Grants
grant select, insert, update, delete on public.taxes, public.categories, public.products, public.product_prices, public.recipes, public.recipe_items to authenticated;
