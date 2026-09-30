-- Capa 1 - Catalogo, surtido, precios e impuestos

create table public.product_categories (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  name text not null,
  sort_order integer not null default 0,
  active boolean not null default true,
  created_at timestamptz not null default now(),
  unique (branch_id, name),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);
create index product_categories_branch_idx on public.product_categories (branch_id);

create table public.products (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  sku text,
  barcode varchar(50),
  name text not null,
  unit_measure_id integer not null references public.tbl_tipos_maestra(tima_id),
  tax_type_id integer references public.tbl_tipos_maestra(tima_id),
  cost_price numeric(12,2),
  base_price numeric(12,2),
  is_composed boolean not null default false,
  active boolean not null default true,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  unique (tenant_id, sku),
  unique (tenant_id, barcode)
);
create index products_tenant_idx on public.products (tenant_id);

create table public.branch_products (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  product_id uuid not null references public.products(id) on delete cascade,
  category_id uuid references public.product_categories(id),
  price numeric(12,2),
  is_available boolean not null default true,
  created_at timestamptz not null default now(),
  unique (branch_id, product_id),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);
create index branch_products_lookup on public.branch_products (branch_id, product_id);

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
alter table public.product_categories enable row level security;
alter table public.products enable row level security;
alter table public.branch_products enable row level security;
alter table public.recipes enable row level security;
alter table public.recipe_items enable row level security;

create policy product_categories_isolation on public.product_categories for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));

create policy products_isolation on public.products for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

create policy branch_products_isolation on public.branch_products for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));

create policy recipes_isolation on public.recipes for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

create policy recipe_items_isolation on public.recipe_items for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

-- Grants
grant select, insert, update, delete on public.product_categories, public.products, public.branch_products, public.recipes, public.recipe_items to authenticated;
