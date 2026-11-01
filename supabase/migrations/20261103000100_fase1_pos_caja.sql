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


-- ============================================================
-- F1.5 Nuevo modelo de líneas y motor de precios
-- ============================================================

alter table public.order_items drop column tax_amount;
alter table public.order_items drop column line_total;
alter table public.order_items
  alter column product_id drop not null,
  add column custom_name text check (custom_name is null or length(trim(custom_name)) between 1 and 80),
  add column tier_id uuid,
  add column units_per_package numeric(14,4) not null default 1 check (units_per_package > 0),
  add column list_price numeric(14,2),
  add column discount_manual numeric(14,2) not null default 0 check (discount_manual >= 0),
  add column discount_promotion numeric(14,2) not null default 0 check (discount_promotion >= 0),
  add column discount_order numeric(14,2) not null default 0 check (discount_order >= 0),
  add column discount_coupon numeric(14,2) not null default 0 check (discount_coupon >= 0),
  add column promotion_id uuid,
  add column price_override_reason text,
  add column line_discount numeric(14,2) generated always as
    (discount_manual + discount_promotion + discount_order + discount_coupon) stored,
  add column line_total numeric(14,2) generated always as
    (round(qty * unit_price, 2) - (discount_manual + discount_promotion + discount_order + discount_coupon)) stored,
  add column tax_amount numeric(14,2) generated always as
    (round((round(qty * unit_price, 2) - (discount_manual + discount_promotion + discount_order + discount_coupon)) * tax_rate, 2)) stored,
  add constraint order_items_product_xor_custom check ((product_id is null) = (custom_name is not null)),
  add constraint order_items_discount_le_gross check
    ((discount_manual + discount_promotion + discount_order + discount_coupon) <= round(qty * unit_price, 2));

alter table public.orders add column discount_total numeric(14,2) not null default 0;

-- ============================================================
-- F1.6 Tarifas y presentaciones
-- ============================================================

create table public.price_tiers (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  name text not null check (length(trim(name)) between 1 and 40),
  kind text not null check (kind in ('CLIENTE','PRESENTACION')),
  adjustment text not null check (adjustment in ('NINGUNO','DESCUENTO_PCT','RECARGO_PCT','DESCUENTO_FIJO','RECARGO_FIJO','PRECIO_FIJO')),
  value numeric(14,4) not null default 0 check (value >= 0),
  units_per_package numeric(14,4) check (units_per_package > 0),
  active boolean not null default true,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  unique (tenant_id, name),
  check (kind = 'PRESENTACION' or units_per_package is null),
  check (adjustment <> 'DESCUENTO_PCT' or value <= 100),
  check (adjustment <> 'RECARGO_PCT' or value <= 1000)
);

create table public.product_tier_prices (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  product_id uuid not null,
  tier_id uuid not null,
  price numeric(14,2) check (price >= 0),
  units_per_package numeric(14,4) check (units_per_package > 0),
  barcode varchar(50),
  primary key (product_id, tier_id),
  foreign key (tenant_id, product_id) references public.products (tenant_id, id) on delete cascade,
  foreign key (tenant_id, tier_id) references public.price_tiers (tenant_id, id)
);

create unique index product_tier_prices_barcode_idx on public.product_tier_prices (tenant_id, barcode) where barcode is not null;

alter table public.price_tiers enable row level security;
alter table public.product_tier_prices enable row level security;

create policy price_tiers_tenant_isolation on public.price_tiers
  for all to authenticated
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

create policy product_tier_prices_tenant_isolation on public.product_tier_prices
  for all to authenticated
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

grant select, insert, update, delete on public.price_tiers to authenticated;
grant select, insert, update, delete on public.product_tier_prices to authenticated;

alter table public.customers add constraint customers_tier_fkey
  foreign key (tenant_id, price_tier_id) references public.price_tiers (tenant_id, id);

alter table public.order_items add constraint order_items_tier_fkey
  foreign key (tenant_id, tier_id) references public.price_tiers (tenant_id, id);

-- F1.7 Promociones
create table public.promotions (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid,                                   -- null = todas las sucursales
  name text not null check (length(trim(name)) between 1 and 60),
  kind text not null check (kind in ('PORCENTAJE','FIJO')),
  value numeric(14,4) not null check (value > 0),
  scope text not null check (scope in ('PEDIDO','PRODUCTO','CATEGORIA')),
  min_qty numeric(14,4) check (min_qty > 0),
  min_subtotal numeric(14,2) check (min_subtotal > 0),
  max_discount numeric(14,2) check (max_discount > 0),
  starts_at timestamptz not null,
  ends_at timestamptz,
  status text not null default 'BORRADOR' check (status in ('BORRADOR','ACTIVA','PAUSADA')),
  priority integer not null default 0,
  usage_limit integer check (usage_limit > 0),
  usage_count integer not null default 0,
  per_customer_limit integer check (per_customer_limit > 0),
  skip_tier_lines boolean not null default true,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  check (ends_at is null or ends_at > starts_at),
  check (kind <> 'PORCENTAJE' or value <= 100),
  check (scope <> 'PEDIDO' or min_qty is null),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);

create table public.promotion_targets (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  promotion_id uuid not null,
  product_id uuid,
  category_id uuid references public.product_categories(id),
  foreign key (tenant_id, promotion_id) references public.promotions (tenant_id, id) on delete cascade,
  foreign key (tenant_id, product_id) references public.products (tenant_id, id),
  check ((product_id is null) <> (category_id is null))
);

create table public.promotion_redemptions (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  promotion_id uuid not null,
  order_id uuid not null,
  customer_id uuid,
  amount numeric(14,2) not null check (amount > 0),
  created_at timestamptz not null default now(),
  unique (promotion_id, order_id),
  foreign key (tenant_id, promotion_id) references public.promotions (tenant_id, id),
  foreign key (tenant_id, order_id) references public.orders (tenant_id, id) on delete cascade
);

alter table public.promotions enable row level security;
alter table public.promotion_targets enable row level security;
alter table public.promotion_redemptions enable row level security;

create policy promotions_tenant_isolation on public.promotions
  for all to authenticated
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

create policy promotion_targets_tenant_isolation on public.promotion_targets
  for all to authenticated
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

create policy promotion_redemptions_tenant_isolation on public.promotion_redemptions
  for all to authenticated
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

grant select, insert, update, delete on public.promotions to authenticated;
grant select, insert, update, delete on public.promotion_targets to authenticated;
grant select, insert, update, delete on public.promotion_redemptions to authenticated;

-- F1.8 Cupones
create table public.coupons (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid,                                   -- null = todas las sucursales
  code text not null check (code ~ '^[A-Z0-9]{4,20}$'),
  name text not null check (length(trim(name)) between 1 and 60),
  kind text not null check (kind in ('PORCENTAJE','FIJO')),
  value numeric(14,4) not null check (value > 0),
  scope text not null check (scope in ('PEDIDO','PRODUCTO','CATEGORIA')),
  min_qty numeric(14,4) check (min_qty > 0),
  min_subtotal numeric(14,2) check (min_subtotal > 0),
  max_discount numeric(14,2) check (max_discount > 0),
  starts_at timestamptz not null,
  ends_at timestamptz,
  status text not null default 'ACTIVO' check (status in ('ACTIVO','INACTIVO')),
  usage_limit integer check (usage_limit > 0),
  usage_count integer not null default 0,
  per_customer_limit integer check (per_customer_limit > 0),
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  unique (tenant_id, code),
  check (ends_at is null or ends_at > starts_at),
  check (kind <> 'PORCENTAJE' or value <= 100),
  check (scope <> 'PEDIDO' or min_qty is null),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);

create table public.coupon_targets (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  coupon_id uuid not null,
  product_id uuid,
  category_id uuid references public.product_categories(id),
  foreign key (tenant_id, coupon_id) references public.coupons (tenant_id, id) on delete cascade,
  foreign key (tenant_id, product_id) references public.products (tenant_id, id),
  check ((product_id is null) <> (category_id is null))
);

create table public.coupon_redemptions (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  coupon_id uuid not null,
  order_id uuid not null unique,
  customer_id uuid,
  amount numeric(14,2) not null check (amount > 0),
  created_at timestamptz not null default now(),
  unique (coupon_id, order_id),
  foreign key (tenant_id, coupon_id) references public.coupons (tenant_id, id),
  foreign key (tenant_id, order_id) references public.orders (tenant_id, id) on delete cascade
);

alter table public.coupons enable row level security;
alter table public.coupon_targets enable row level security;
alter table public.coupon_redemptions enable row level security;

create policy coupons_tenant_isolation on public.coupons
  for all to authenticated
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

create policy coupon_targets_tenant_isolation on public.coupon_targets
  for all to authenticated
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

create policy coupon_redemptions_tenant_isolation on public.coupon_redemptions
  for all to authenticated
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

grant select, insert, update, delete on public.coupons to authenticated;
grant select, insert, update, delete on public.coupon_targets to authenticated;
grant select, insert, update, delete on public.coupon_redemptions to authenticated;

-- ============================================================
-- F1.9 Cambio de precio e ítem libre
-- ============================================================

alter table public.products add column allow_price_override boolean not null default true;

-- ============================================================
-- F1.11 Devoluciones
-- ============================================================

insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select 'Devolucion de cliente', 'DEVOLUCION', h.tima_id from public.tbl_tipos_maestra h
where h.padre_id is null and h.tima_codigo = 'MOV'
on conflict (padre_id, tima_codigo) do nothing;

create table public.refunds (
  id uuid primary key,
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  order_id uuid not null,
  prefix text not null,
  number bigint not null,
  status text not null check (status in ('PENDIENTE','APROBADA','RECHAZADA')),
  method text not null check (method in ('EFECTIVO','ORIGINAL')),
  original_method text,
  reversal_reference text,
  reason text not null check (length(trim(reason)) between 3 and 200),
  subtotal numeric(14,2) not null,
  tax_amount numeric(14,2) not null,
  total numeric(14,2) not null check (total > 0),
  cash_session_id uuid,
  requested_by uuid not null references public.users(id),
  decided_by uuid references public.users(id),
  decided_at timestamptz,
  reject_reason text,
  client_request jsonb not null,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  unique (branch_id, prefix, number),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id),
  foreign key (tenant_id, order_id) references public.orders (tenant_id, id),
  foreign key (tenant_id, cash_session_id) references public.cash_sessions (tenant_id, id)
);
create index refunds_order_idx on public.refunds (order_id);
create index refunds_branch_status_idx on public.refunds (branch_id, status);

create table public.refund_items (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  refund_id uuid not null,
  order_item_id uuid not null,
  qty numeric(14,4) not null check (qty > 0),
  disposition text not null check (disposition in ('REINTEGRAR','MERMA','SIN_DEVOLUCION')),
  line_total numeric(14,2) not null,
  tax_amount numeric(14,2) not null,
  foreign key (tenant_id, refund_id) references public.refunds (tenant_id, id) on delete cascade,
  foreign key (tenant_id, order_item_id) references public.order_items (tenant_id, id)
);
create index refund_items_refund_idx on public.refund_items (refund_id);
create index refund_items_order_item_idx on public.refund_items (order_item_id);

alter table public.orders add column refunded_amount numeric(14,2) not null default 0;

alter table public.fiscal_documents add column refund_id uuid;
alter table public.fiscal_documents drop constraint fiscal_documents_order_id_doc_type_key;
create unique index fiscal_documents_order_doc_idx on public.fiscal_documents (order_id, doc_type) where refund_id is null;
create unique index fiscal_documents_refund_idx on public.fiscal_documents (refund_id) where refund_id is not null;

alter table public.refunds enable row level security;
alter table public.refund_items enable row level security;
create policy refunds_isolation on public.refunds for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));
create policy refund_items_isolation on public.refund_items for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());
grant select, insert, update, delete on public.refunds, public.refund_items to authenticated;

-- ============================================================
-- F1.12 Anulación de ventas
-- ============================================================

insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select 'Anulado', 'ANULADO', h.tima_id from public.tbl_tipos_maestra h
where h.padre_id is null and h.tima_codigo = 'DOCF'
on conflict (padre_id, tima_codigo) do nothing;

alter table public.orders add column void_reason text;
