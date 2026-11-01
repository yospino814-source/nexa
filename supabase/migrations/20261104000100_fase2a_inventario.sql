-- ============================================================
-- F2.1 Campos de producto y unidades
-- ============================================================

insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.n, c.c, h.tima_id from (values ('Kilogramo','KG'),('Litro','L'),('Metro','M'),
  ('Caja','CAJA'),('Paquete','PAQUETE'),('Bulto','BULTO'),('Libra','LB')) c(n, c)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'UOM') h
on conflict (padre_id, tima_codigo) do nothing;

alter table public.products
  add column kind text not null default 'FISICO' check (kind in ('FISICO','SERVICIO','PREPARADO')),
  add column usage text not null default 'VENTA' check (usage in ('VENTA','INSUMO','INTERNO')),
  add column purchase_uom_id integer references public.tbl_tipos_maestra(tima_id),
  add column purchase_factor numeric(14,4) not null default 1 check (purchase_factor > 0),
  add column reorder_qty numeric(14,4) check (reorder_qty > 0),
  add column max_qty numeric(14,4) check (max_qty > 0),
  add column track_lots boolean not null default false,
  add column parent_id uuid,
  add column has_variants boolean not null default false,
  add column variant_attrs jsonb,
  add column archived_at timestamptz,
  add column description text check (length(description) <= 500),
  add constraint products_parent_fkey foreign key (tenant_id, parent_id) references public.products (tenant_id, id),
  add constraint products_variant_shape check (
    (parent_id is null) or (has_variants = false and variant_attrs is not null)),
  add constraint products_service_no_lots check (kind <> 'SERVICIO' or track_lots = false);

-- ============================================================
-- F2.4 Modificadores
-- ============================================================

create table public.option_groups (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  name text not null check (length(trim(name)) between 1 and 40),
  min_select integer not null default 0 check (min_select >= 0),
  max_select integer not null default 1 check (max_select >= 1),
  active boolean not null default true,
  unique (tenant_id, id), unique (tenant_id, name),
  check (min_select <= max_select)
);

create table public.options (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  group_id uuid not null,
  name text not null check (length(trim(name)) between 1 and 40),
  price_delta numeric(14,2) not null default 0 check (price_delta >= 0),
  input_product_id uuid,
  input_qty numeric(14,4) check (input_qty > 0),
  sort_order integer not null default 0,
  active boolean not null default true,
  unique (tenant_id, id),
  foreign key (tenant_id, group_id) references public.option_groups (tenant_id, id) on delete cascade,
  foreign key (tenant_id, input_product_id) references public.products (tenant_id, id),
  check ((input_product_id is null) = (input_qty is null))
);

create table public.product_option_groups (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  product_id uuid not null,
  group_id uuid not null,
  sort_order integer not null default 0,
  primary key (product_id, group_id),
  foreign key (tenant_id, product_id) references public.products (tenant_id, id) on delete cascade,
  foreign key (tenant_id, group_id) references public.option_groups (tenant_id, id)
);

create table public.order_item_options (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  order_item_id uuid not null,
  option_id uuid not null,
  name text not null,
  price_delta numeric(14,2) not null,
  primary key (order_item_id, option_id),
  foreign key (tenant_id, order_item_id) references public.order_items (tenant_id, id) on delete cascade,
  foreign key (tenant_id, option_id) references public.options (tenant_id, id)
);

alter table public.option_groups enable row level security;
alter table public.options enable row level security;
alter table public.product_option_groups enable row level security;
alter table public.order_item_options enable row level security;

create policy option_groups_tenant_isolation on public.option_groups
  for all to authenticated
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

create policy options_tenant_isolation on public.options
  for all to authenticated
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

create policy product_option_groups_tenant_isolation on public.product_option_groups
  for all to authenticated
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

create policy order_item_options_tenant_isolation on public.order_item_options
  for all to authenticated
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

grant select, insert, update, delete on public.option_groups to authenticated;
grant select, insert, update, delete on public.options to authenticated;
grant select, insert, update, delete on public.product_option_groups to authenticated;
grant select, insert, update, delete on public.order_item_options to authenticated;

-- F2.6 Importación CSV en dos pasos
create table public.import_sessions (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  created_by uuid not null references public.users(id),
  rows jsonb not null,
  committed_rows integer not null default 0,
  created_at timestamptz not null default now(),
  expires_at timestamptz not null default now() + interval '24 hours',
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);

alter table public.import_sessions enable row level security;

create policy import_sessions_tenant_isolation on public.import_sessions
  for all to authenticated
  using (in_scope(tenant_id, branch_id))
  with check (in_scope(tenant_id, branch_id));

grant select, insert, update on public.import_sessions to authenticated;

-- ============================================================
-- F2.7 Etiquetas con código de barras
-- ============================================================

create table public.tenant_counters (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  name text not null,
  next_value bigint not null default 1,
  primary key (tenant_id, name)
);

alter table public.tenant_counters enable row level security;

create policy tenant_counters_tenant_isolation on public.tenant_counters
  for all to authenticated
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

grant select, insert, update on public.tenant_counters to authenticated;

-- ============================================================
-- F2.8 Bodegas (ubicaciones)
-- ============================================================

create table public.locations (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  name text not null check (length(trim(name)) between 1 and 40),
  kind text not null check (kind in ('PISO_VENTA','BODEGA','PRODUCCION','RECEPCION','DESPACHO','CUARENTENA','AVERIADOS')),
  sellable boolean not null,
  is_default boolean not null default false,
  active boolean not null default true,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  unique (tenant_id, branch_id, id),
  unique (branch_id, name),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id),
  check (not (kind in ('CUARENTENA','AVERIADOS') and sellable)),
  check (not is_default or (sellable and active))
);
create unique index locations_one_default on public.locations (branch_id) where is_default;

alter table public.locations enable row level security;

create policy locations_tenant_isolation on public.locations
  for all to authenticated
  using (in_scope(tenant_id, branch_id))
  with check (in_scope(tenant_id, branch_id));

grant select, insert, update on public.locations to authenticated;

insert into public.locations (tenant_id, branch_id, name, kind, sellable, is_default)
select tenant_id, id, 'Principal', 'PISO_VENTA', true, true from public.branches;

create or replace function public.branch_default_register() returns trigger language plpgsql as $$
declare
  v_loc_id uuid;
begin
  insert into public.locations (tenant_id, branch_id, name, kind, sellable, is_default)
  values (new.tenant_id, new.id, 'Principal', 'PISO_VENTA', true, true)
  returning id into v_loc_id;

  insert into public.cash_registers (tenant_id, branch_id, name, code, location_id)
  values (new.tenant_id, new.id, 'Caja 1', 'CAJA-1', v_loc_id);
  return new;
end $$;

alter table public.stock add column location_id uuid;
update public.stock s set location_id = l.id from public.locations l where l.branch_id = s.branch_id and l.is_default;
alter table public.stock alter column location_id set not null;
alter table public.stock add constraint stock_location_fkey
  foreign key (tenant_id, branch_id, location_id) references public.locations (tenant_id, branch_id, id);
alter table public.stock drop constraint stock_branch_id_product_id_key;
alter table public.stock add constraint stock_location_product_key unique (location_id, product_id);

alter table public.inventory_movements add column location_id uuid;
update public.inventory_movements m set location_id = l.id from public.locations l where l.branch_id = m.branch_id and l.is_default;
alter table public.inventory_movements alter column location_id set not null;
alter table public.inventory_movements add constraint inventory_movements_location_fkey
  foreign key (tenant_id, branch_id, location_id) references public.locations (tenant_id, branch_id, id);

alter table public.stock_reservations add column location_id uuid;
update public.stock_reservations r set location_id = l.id from public.locations l where l.branch_id = r.branch_id and l.is_default;
alter table public.stock_reservations alter column location_id set not null;
alter table public.stock_reservations add constraint stock_reservations_location_fkey
  foreign key (tenant_id, branch_id, location_id) references public.locations (tenant_id, branch_id, id);

alter table public.cash_registers add column location_id uuid,
  add constraint cash_registers_location_fkey foreign key (tenant_id, branch_id, location_id) references public.locations (tenant_id, branch_id, id);
update public.cash_registers c set location_id = l.id from public.locations l where l.branch_id = c.branch_id and l.is_default;


