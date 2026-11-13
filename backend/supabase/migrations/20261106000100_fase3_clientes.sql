-- Migración única para Fase 3: Clientes, cartera (fiado), separados, cotizaciones y saldo a favor
-- 20261106000100_fase3_clientes.sql

-- 1. Permisos nuevos y asignación a roles
insert into public.permissions (code, module, description) values
  ('credit:override', 'credit', 'Autorizar crédito por encima del cupo o con mora'),
  ('wallet:adjust', 'customers', 'Ajustar el saldo a favor de un cliente')
on conflict (code) do nothing;

-- DUENO y ADMIN_CAJA reciben credit:override y wallet:adjust
insert into public.role_permissions (role_id, permission_code)
select r.tima_id, p.code
from public.tbl_tipos_maestra r
cross join public.permissions p
where r.padre_id = (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
  and r.tima_codigo in ('DUENO', 'ADMIN_CAJA')
  and p.code in ('credit:override', 'wallet:adjust')
on conflict do nothing;

-- SUPERVISOR recibe credit:override
insert into public.role_permissions (role_id, permission_code)
select r.tima_id, p.code
from public.tbl_tipos_maestra r
cross join public.permissions p
where r.padre_id = (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
  and r.tima_codigo = 'SUPERVISOR'
  and p.code = 'credit:override'
on conflict do nothing;

-- SUPERVISOR recibe credit:sell
insert into public.role_permissions (role_id, permission_code)
select r.tima_id, p.code
from public.tbl_tipos_maestra r
cross join public.permissions p
where r.padre_id = (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
  and r.tima_codigo = 'SUPERVISOR'
  and p.code = 'credit:sell'
on conflict do nothing;

-- CAJERO recibe credit:sell
insert into public.role_permissions (role_id, permission_code)
select r.tima_id, p.code
from public.tbl_tipos_maestra r
cross join public.permissions p
where r.padre_id = (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
  and r.tima_codigo = 'CAJERO'
  and p.code = 'credit:sell'
on conflict do nothing;

-- 2. F3.1 Clientes completos
alter table public.customers
  add column if not exists person_type text not null default 'NATURAL' check (person_type in ('NATURAL','JURIDICA')),
  add column if not exists tax_regime text not null default 'NO_RESPONSABLE' check (tax_regime in ('RESPONSABLE_IVA','NO_RESPONSABLE')),
  add column if not exists fiscal_responsibilities text[] not null default '{R-99-PN}',
  add column if not exists ciiu text check (ciiu ~ '^[0-9]{4}$'),
  add column if not exists withholding_agent boolean not null default false,
  add column if not exists city_code text check (city_code ~ '^[0-9]{5}$'),
  add column if not exists billing_address text check (length(billing_address) <= 200),
  add column if not exists shipping_address text check (length(shipping_address) <= 200),
  add column if not exists pinned_note text check (length(pinned_note) <= 300),
  add column if not exists credit_limit numeric(14,2) not null default 0 check (credit_limit >= 0),
  add column if not exists credit_block_days integer not null default 30 check (credit_block_days between 0 and 365);

do $$
begin
  if not exists (select 1 from pg_constraint where conname = 'customers_resp_codes') then
    alter table public.customers
      add constraint customers_resp_codes check (fiscal_responsibilities <@ array['O-13','O-15','O-23','O-47','R-99-PN']::text[]);
  end if;
end $$;

-- 3. F3.2 Venta a crédito y cuentas por cobrar
insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select 'Credito', 'CREDITO', h.tima_id from public.tbl_tipos_maestra h
where h.padre_id is null and h.tima_codigo = 'PAGO'
on conflict (padre_id, tima_codigo) do nothing;

create table if not exists public.receivables (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  customer_id uuid not null,
  source text not null check (source in ('VENTA','ACUERDO')),
  order_id uuid,
  prefix text, number bigint,
  principal numeric(14,2) not null check (principal >= 0),
  interest_total numeric(14,2) not null default 0 check (interest_total >= 0),
  amount_total numeric(14,2) not null,
  amount_paid numeric(14,2) not null default 0 check (amount_paid >= 0),
  status text not null default 'ABIERTA' check (status in ('ABIERTA','PARCIAL','PAGADA','CASTIGADA','REFINANCIADA')),
  due_date date not null,
  plan jsonb,
  written_off_reason text,
  refinanced_into uuid,
  created_by uuid not null references public.users(id),
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  unique (order_id),
  check (amount_total = principal + interest_total),
  check (amount_paid <= amount_total),
  check ((source = 'VENTA') = (order_id is not null)),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id),
  foreign key (tenant_id, customer_id) references public.customers (tenant_id, id),
  foreign key (tenant_id, order_id) references public.orders (tenant_id, id)
);

create table if not exists public.receivable_installments (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  receivable_id uuid not null,
  n integer not null check (n >= 1),
  due_date date not null,
  amount numeric(14,2) not null check (amount > 0),
  amount_paid numeric(14,2) not null default 0 check (amount_paid >= 0),
  primary key (receivable_id, n),
  check (amount_paid <= amount),
  foreign key (tenant_id, receivable_id) references public.receivables (tenant_id, id) on delete cascade
);

alter table public.receivables enable row level security;
drop policy if exists receivables_isolation on public.receivables;
create policy receivables_isolation on public.receivables for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));
grant select, insert, update, delete on public.receivables to authenticated;

alter table public.receivable_installments enable row level security;
drop policy if exists receivable_installments_isolation on public.receivable_installments;
create policy receivable_installments_isolation on public.receivable_installments for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());
grant select, insert, update, delete on public.receivable_installments to authenticated;

create or replace function public.customer_credit_position(p_customer uuid)
returns table (balance numeric(14,2), max_days_overdue integer)
language sql
stable
security definer
set search_path = ''
as $$
  with x as (
    select r.amount_total - r.amount_paid as saldo,
           greatest(0, (now() at time zone b.timezone)::date - coalesce(i.first_due, r.due_date)) as dias
      from public.receivables r
      join public.branches b on b.id = r.branch_id
      left join lateral (
        select min(ri.due_date) as first_due
          from public.receivable_installments ri
         where ri.receivable_id = r.id and ri.amount_paid < ri.amount
      ) i on true
     where r.tenant_id = public.current_tenant_id()
       and r.customer_id = p_customer
       and r.status in ('ABIERTA','PARCIAL')
  )
  select coalesce(sum(saldo), 0)::numeric(14,2), coalesce(max(dias), 0)::integer from x
$$;
revoke all on function public.customer_credit_position(uuid) from public, anon;
grant execute on function public.customer_credit_position(uuid) to authenticated;

-- 4. F3.3 Abonos y cuotas
create table if not exists public.receivable_payments (
  id uuid primary key,
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  receivable_id uuid not null,
  amount numeric(14,2) not null check (amount > 0),
  method text not null check (method in ('EFECTIVO','DATAFONO','QR','TRANSFERENCIA','SALDO')),
  reference text,
  cash_session_id uuid,
  allocation jsonb not null,
  created_by uuid not null references public.users(id),
  client_request jsonb not null,
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id),
  foreign key (tenant_id, receivable_id) references public.receivables (tenant_id, id),
  foreign key (tenant_id, cash_session_id) references public.cash_sessions (tenant_id, id)
);

alter table public.receivable_payments enable row level security;
drop policy if exists receivable_payments_isolation on public.receivable_payments;
create policy receivable_payments_isolation on public.receivable_payments for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));
grant select, insert, update, delete on public.receivable_payments to authenticated;

create index if not exists receivables_customer_idx on public.receivables (tenant_id, customer_id) where status in ('ABIERTA','PARCIAL');
create index if not exists receivable_payments_receivable_idx on public.receivable_payments (receivable_id, created_at);

-- 5. F3.6 Saldo a favor (billetera)
create table if not exists public.customer_wallets (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  customer_id uuid primary key,
  balance numeric(14,2) not null default 0 check (balance >= 0),
  foreign key (tenant_id, customer_id) references public.customers (tenant_id, id)
);

create table if not exists public.wallet_movements (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  customer_id uuid not null,
  kind text not null check (kind in ('RECARGA','USO','DEVOLUCION','AJUSTE','SEPARADO')),
  amount numeric(14,2) not null check (amount <> 0),
  balance_before numeric(14,2) not null,
  balance_after numeric(14,2) not null,
  ref_type text, ref_id uuid,
  reason text,
  created_by uuid not null references public.users(id),
  created_at timestamptz not null default now(),
  check (balance_after = balance_before + amount),
  foreign key (tenant_id, customer_id) references public.customers (tenant_id, id)
);

insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select 'Saldo a favor', 'SALDO', h.tima_id from public.tbl_tipos_maestra h
where h.padre_id is null and h.tima_codigo = 'PAGO'
on conflict (padre_id, tima_codigo) do nothing;

do $$
begin
  if exists (select 1 from pg_constraint where conname = 'refunds_method_check') then
    alter table public.refunds drop constraint refunds_method_check;
  end if;
  alter table public.refunds add constraint refunds_method_check check (method in ('EFECTIVO','ORIGINAL','SALDO_A_FAVOR'));
end $$;

alter table public.refunds
  add column if not exists receivable_applied numeric(14,2) not null default 0 check (receivable_applied >= 0);

alter table public.customer_wallets enable row level security;
drop policy if exists customer_wallets_isolation on public.customer_wallets;
create policy customer_wallets_isolation on public.customer_wallets for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());
grant select, insert, update, delete on public.customer_wallets to authenticated;

alter table public.wallet_movements enable row level security;
drop policy if exists wallet_movements_isolation on public.wallet_movements;
create policy wallet_movements_isolation on public.wallet_movements for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());
grant select, insert, update, delete on public.wallet_movements to authenticated;

-- 6. F3.7 Separados
insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select 'Separado', 'SEPARADO', h.tima_id from public.tbl_tipos_maestra h
where h.padre_id is null and h.tima_codigo = 'VENTA'
on conflict (padre_id, tima_codigo) do nothing;

create table if not exists public.layaways (
  id uuid primary key,
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  prefix text not null, number bigint not null,
  customer_id uuid not null,
  status text not null default 'ACTIVO' check (status in ('ACTIVO','COMPLETADO','CANCELADO','VENCIDO')),
  total numeric(14,2) not null check (total > 0),
  amount_paid numeric(14,2) not null default 0 check (amount_paid >= 0),
  expires_on date not null,
  penalty numeric(14,2),
  refunded numeric(14,2),
  created_by uuid not null references public.users(id),
  client_request jsonb not null,
  created_at timestamptz not null default now(),
  unique (tenant_id, id), unique (branch_id, prefix, number),
  check (amount_paid <= total),
  foreign key (tenant_id, id) references public.orders (tenant_id, id),
  foreign key (tenant_id, customer_id) references public.customers (tenant_id, id)
);

create table if not exists public.layaway_installments (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  layaway_id uuid not null,
  n integer not null check (n >= 0),
  due_date date not null,
  amount numeric(14,2) not null check (amount > 0),
  amount_paid numeric(14,2) not null default 0,
  primary key (layaway_id, n),
  check (amount_paid <= amount),
  foreign key (tenant_id, layaway_id) references public.layaways (tenant_id, id) on delete cascade
);

create table if not exists public.layaway_payments (
  id uuid primary key,
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  layaway_id uuid not null,
  amount numeric(14,2) not null check (amount > 0),
  method text not null check (method in ('EFECTIVO','DATAFONO','QR','TRANSFERENCIA','SALDO')),
  reference text,
  cash_session_id uuid,
  created_by uuid not null references public.users(id),
  client_request jsonb not null,
  created_at timestamptz not null default now(),
  foreign key (tenant_id, layaway_id) references public.layaways (tenant_id, id)
);

alter table public.orders add column if not exists layaway_id uuid;

alter table public.layaways enable row level security;
drop policy if exists layaways_isolation on public.layaways;
create policy layaways_isolation on public.layaways for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));
grant select, insert, update, delete on public.layaways to authenticated;

alter table public.layaway_installments enable row level security;
drop policy if exists layaway_installments_isolation on public.layaway_installments;
create policy layaway_installments_isolation on public.layaway_installments for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());
grant select, insert, update, delete on public.layaway_installments to authenticated;

alter table public.layaway_payments enable row level security;
drop policy if exists layaway_payments_isolation on public.layaway_payments;
create policy layaway_payments_isolation on public.layaway_payments for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());
grant select, insert, update, delete on public.layaway_payments to authenticated;

-- 7. F3.8 Cotizaciones
create table if not exists public.quotes (
  id uuid primary key,
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  prefix text not null, number bigint not null,
  customer_id uuid,
  customer_name text,
  customer_phone text,
  status text not null default 'BORRADOR' check (status in ('BORRADOR','ENVIADA','ACEPTADA','RECHAZADA','CONVERTIDA')),
  valid_until date not null,
  notes text check (length(notes) <= 500),
  subtotal numeric(14,2) not null, discount_total numeric(14,2) not null,
  tax_total numeric(14,2) not null, total numeric(14,2) not null,
  lines jsonb not null,
  request jsonb not null,
  order_id uuid,
  created_by uuid not null references public.users(id),
  created_at timestamptz not null default now(),
  unique (tenant_id, id), unique (branch_id, prefix, number),
  check (customer_id is not null or customer_name is not null),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id),
  foreign key (tenant_id, customer_id) references public.customers (tenant_id, id)
);

alter table public.orders add column if not exists quote_id uuid;

alter table public.quotes enable row level security;
drop policy if exists quotes_isolation on public.quotes;
create policy quotes_isolation on public.quotes for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));
grant select, insert, update, delete on public.quotes to authenticated;
