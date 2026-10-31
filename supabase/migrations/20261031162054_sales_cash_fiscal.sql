-- Capa 4 - Venta de mostrador: pagos, turnos de caja, numeracion y outbox fiscal

-- ============================================================
-- Tipos maestros nuevos
-- ============================================================
insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.nombre, c.codigo, h.tima_id
from (values ('Dueño','DUENO'),('Cajero','CAJERO')) c(nombre, codigo)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL') h;

insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id) values
  ('TIPOS DE METODO DE PAGO', 'PAGO', null),
  ('TIPOS DE ESTADO DE DOCUMENTO FISCAL', 'DOCF', null);

insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.nombre, c.codigo, h.tima_id
from (values ('Efectivo','EFECTIVO'),('Datafono','DATAFONO'),('QR / Nequi','QR')) c(nombre, codigo)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'PAGO') h;

insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.nombre, c.codigo, h.tima_id
from (values ('Pendiente','PENDIENTE'),('Enviado','ENVIADO'),('Aceptado','ACEPTADO'),
             ('Rechazado','RECHAZADO')) c(nombre, codigo)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'DOCF') h;

-- ============================================================
-- Turnos de caja (apertura, arqueo y cierre)
-- ============================================================
create table public.cash_sessions (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  opened_by uuid not null references public.users(id),
  opening_amount numeric(14,2) not null check (opening_amount >= 0),
  opened_at timestamptz not null default now(),
  closed_by uuid references public.users(id),
  closed_at timestamptz,
  -- Calculados al cerrar: base + efectivo de las ventas del turno.
  expected_cash numeric(14,2),
  counted_cash numeric(14,2) check (counted_cash is null or counted_cash >= 0),
  difference numeric(14,2),
  notes text,
  unique (tenant_id, id),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id),
  check ((closed_at is null) = (counted_cash is null))
);
-- Un cajero tiene a lo sumo un turno abierto por sucursal.
create unique index cash_sessions_one_open_idx
  on public.cash_sessions (branch_id, opened_by) where closed_at is null;
create index cash_sessions_branch_idx on public.cash_sessions (tenant_id, branch_id, opened_at desc);

-- ============================================================
-- Numeracion consecutiva sin huecos (documento equivalente POS)
-- ============================================================
-- Se incrementa con bloqueo de fila dentro de la transaccion de la venta:
-- si la venta se revierte, el numero no se consume (una secuencia de
-- Postgres si dejaria huecos).
create table public.document_sequences (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  doc_type text not null check (doc_type in ('POS')),
  prefix text not null,
  next_number bigint not null default 1 check (next_number > 0),
  -- Rango y resolucion autorizados por la DIAN (null = sin autorizar aun).
  range_to bigint check (range_to is null or range_to > 0),
  resolution_number text,
  resolution_date date,
  valid_until date,
  created_at timestamptz not null default now(),
  unique (branch_id, doc_type),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);

-- ============================================================
-- Datos de venta en la orden
-- ============================================================
alter table public.orders
  add column cash_session_id uuid,
  add column receipt_prefix text,
  add column receipt_number bigint,
  add column paid_amount numeric(14,2) not null default 0,
  add column change_due numeric(14,2) not null default 0 check (change_due >= 0),
  -- Cuerpo original de POST /v1/sales: un reintento con el mismo id debe
  -- traer exactamente el mismo cuerpo (idempotencia).
  add column client_request jsonb,
  add foreign key (tenant_id, cash_session_id) references public.cash_sessions (tenant_id, id);
create unique index orders_receipt_idx
  on public.orders (branch_id, receipt_prefix, receipt_number) where receipt_number is not null;
create index orders_cash_session_idx on public.orders (cash_session_id);

-- ============================================================
-- Pagos (varios por orden: efectivo + datafono, etc.)
-- ============================================================
create table public.payments (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  order_id uuid not null,
  cash_session_id uuid,
  method_id integer not null references public.tbl_tipos_maestra(tima_id),
  -- Monto aplicado a la orden. En efectivo, `tendered` es lo recibido.
  amount numeric(14,2) not null check (amount > 0),
  tendered numeric(14,2) check (tendered is null or tendered >= amount),
  reference text,
  created_by uuid references auth.users(id),
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id),
  foreign key (tenant_id, order_id) references public.orders (tenant_id, id) on delete cascade,
  foreign key (tenant_id, cash_session_id) references public.cash_sessions (tenant_id, id)
);
create index payments_order_idx on public.payments (order_id);
create index payments_session_idx on public.payments (cash_session_id);

-- ============================================================
-- Outbox de documentos fiscales (DIAN via proveedor tecnologico)
-- ============================================================
-- La venta inserta el documento PENDIENTE en su misma transaccion; un worker
-- aparte lo transmite al proveedor, reintenta y guarda el CUDE. Asi una caida
-- del proveedor nunca bloquea la caja.
create table public.fiscal_documents (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  order_id uuid not null,
  doc_type text not null default 'POS',
  prefix text not null,
  number bigint not null,
  status_id integer not null references public.tbl_tipos_maestra(tima_id),
  cude text,
  provider_response jsonb,
  attempts integer not null default 0,
  last_error text,
  next_attempt_at timestamptz not null default now(),
  created_at timestamptz not null default now(),
  updated_at timestamptz not null default now(),
  unique (order_id, doc_type),
  unique (branch_id, doc_type, prefix, number),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id),
  foreign key (tenant_id, order_id) references public.orders (tenant_id, id) on delete cascade
);
create index fiscal_documents_queue_idx on public.fiscal_documents (status_id, next_attempt_at);

-- ============================================================
-- Row-Level Security y grants
-- ============================================================
alter table public.cash_sessions enable row level security;
alter table public.document_sequences enable row level security;
alter table public.payments enable row level security;
alter table public.fiscal_documents enable row level security;

create policy cash_sessions_isolation on public.cash_sessions for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));

create policy document_sequences_isolation on public.document_sequences for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));

create policy payments_isolation on public.payments for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));

create policy fiscal_documents_isolation on public.fiscal_documents for all
  using (public.in_scope(tenant_id, branch_id))
  with check (public.in_scope(tenant_id, branch_id));

grant select, insert, update, delete on public.cash_sessions, public.document_sequences,
  public.payments, public.fiscal_documents to authenticated;
