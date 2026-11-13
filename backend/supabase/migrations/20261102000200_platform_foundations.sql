-- Capa 0: Plataforma (Fase 0)
-- Permisos granulares, ajustes por sucursal, auditoría inmutable, secuencias unificadas y outbox.

-- ============================================================
-- F0.1 Permisos granulares y overrides por tenant
-- ============================================================
create table public.permissions (
  code text primary key,
  module text not null,
  description text not null
);

create table public.role_permissions (
  role_id integer not null references public.tbl_tipos_maestra(tima_id) on delete cascade,
  permission_code text not null references public.permissions(code) on delete cascade,
  primary key (role_id, permission_code)
);

create table public.tenant_role_overrides (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  role_id integer not null references public.tbl_tipos_maestra(tima_id) on delete cascade,
  permission_code text not null references public.permissions(code) on delete cascade,
  allowed boolean not null,
  created_at timestamptz not null default now(),
  primary key (tenant_id, role_id, permission_code)
);

-- Semilla de roles de sistema adicionales si faltan
insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.nombre, c.codigo, h.tima_id
from (values
  ('Supervisor', 'SUPERVISOR'),
  ('Bodega', 'BODEGA'),
  ('Repartidor', 'REPARTIDOR')
) c(nombre, codigo)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL') h
on conflict (padre_id, tima_codigo) do nothing;

-- Semilla de permisos del catálogo
insert into public.permissions (code, module, description) values
  ('pos:sell', 'pos', 'Vender en mostrador y mesas'),
  ('pos:price_override', 'pos', 'Modificar precio unitario en venta'),
  ('pos:discount', 'pos', 'Aplicar descuento en venta'),
  ('pos:custom_item', 'pos', 'Agregar ítem personalizado'),
  ('pos:refund', 'pos', 'Anular o devolver ventas'),
  ('pos:order', 'pos', 'Tomar pedidos y comandas de mesa'),
  ('pos:kitchen', 'pos', 'Ver y preparar pedidos en pantalla de cocina'),
  ('cash:open', 'cash', 'Abrir turno de caja'),
  ('cash:close', 'cash', 'Cerrar turno y realizar arqueo de caja'),
  ('cash:movement', 'cash', 'Registrar ingresos y egresos de caja'),
  ('cash:history', 'cash', 'Ver historial y cortes de caja'),
  ('inventory:read', 'inventory', 'Consultar catálogo y existencias'),
  ('inventory:adjust', 'inventory', 'Ajustar inventario y conteo físico'),
  ('inventory:adjust_approve', 'inventory', 'Aprobar ajustes de inventario'),
  ('inventory:transfer', 'inventory', 'Trasladar existencias entre sucursales'),
  ('purchases:create', 'purchases', 'Registrar compras a proveedores'),
  ('purchases:approve', 'purchases', 'Aprobar compras a proveedores'),
  ('purchases:receive', 'purchases', 'Recibir mercancía de compras'),
  ('purchases:pay', 'purchases', 'Pagar facturas de compras'),
  ('customers:read', 'customers', 'Consultar directorio de clientes'),
  ('customers:write', 'customers', 'Crear y editar clientes'),
  ('credit:sell', 'credit', 'Vender a crédito / fiado'),
  ('credit:collect', 'credit', 'Cobrar abonos de crédito'),
  ('credit:write_off', 'credit', 'Castigar cartera incobrable'),
  ('reports:read', 'reports', 'Consultar reportes y estadísticas'),
  ('settings:write', 'settings', 'Modificar configuración del negocio y sucursales'),
  ('users:manage', 'users', 'Administrar usuarios y roles')
on conflict (code) do update set description = excluded.description, module = excluded.module;

-- Permisos base para DUENO y ADMIN_CAJA (todos)
insert into public.role_permissions (role_id, permission_code)
select r.tima_id, p.code
from public.tbl_tipos_maestra r
cross join public.permissions p
where r.padre_id = (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
  and r.tima_codigo in ('DUENO', 'ADMIN_CAJA')
on conflict do nothing;

-- Permisos base para CAJERO
insert into public.role_permissions (role_id, permission_code)
select r.tima_id, p.code
from public.tbl_tipos_maestra r
join public.permissions p on p.code in (
  'pos:sell', 'pos:order', 'pos:discount', 'pos:custom_item',
  'cash:open', 'cash:close', 'cash:movement', 'cash:history',
  'inventory:read', 'customers:read', 'customers:write', 'credit:collect'
)
where r.padre_id = (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
  and r.tima_codigo = 'CAJERO'
on conflict do nothing;

-- Permisos base para MESERO
insert into public.role_permissions (role_id, permission_code)
select r.tima_id, p.code
from public.tbl_tipos_maestra r
join public.permissions p on p.code in ('pos:order', 'inventory:read', 'customers:read')
where r.padre_id = (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
  and r.tima_codigo = 'MESERO'
on conflict do nothing;

-- Permisos base para COCINA
insert into public.role_permissions (role_id, permission_code)
select r.tima_id, p.code
from public.tbl_tipos_maestra r
join public.permissions p on p.code in ('pos:kitchen', 'inventory:read')
where r.padre_id = (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
  and r.tima_codigo = 'COCINA'
on conflict do nothing;

-- Permisos base para SUPERVISOR (los de cajero más pos:refund, inventory:adjust, reports:read)
insert into public.role_permissions (role_id, permission_code)
select r.tima_id, p.code
from public.tbl_tipos_maestra r
join public.permissions p on p.code in (
  'pos:sell', 'pos:order', 'pos:discount', 'pos:custom_item',
  'cash:open', 'cash:close', 'cash:movement', 'cash:history',
  'inventory:read', 'customers:read', 'customers:write', 'credit:collect',
  'pos:refund', 'inventory:adjust', 'reports:read'
)
where r.padre_id = (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
  and r.tima_codigo = 'SUPERVISOR'
on conflict do nothing;

-- Permisos base para BODEGA (inventory:* y purchases:receive)
insert into public.role_permissions (role_id, permission_code)
select r.tima_id, p.code
from public.tbl_tipos_maestra r
join public.permissions p on p.code in (
  'inventory:read', 'inventory:adjust', 'inventory:adjust_approve', 'inventory:transfer',
  'purchases:receive'
)
where r.padre_id = (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
  and r.tima_codigo = 'BODEGA'
on conflict do nothing;

-- Permisos base para REPARTIDOR (pos:sell e inventory:read)
insert into public.role_permissions (role_id, permission_code)
select r.tima_id, p.code
from public.tbl_tipos_maestra r
join public.permissions p on p.code in (
  'pos:sell', 'inventory:read'
)
where r.padre_id = (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
  and r.tima_codigo = 'REPARTIDOR'
on conflict do nothing;

-- ============================================================
-- F0.2 Usuarios, invitaciones y membresías
-- ============================================================
alter table public.users add column if not exists email text;
alter table public.memberships add column if not exists active boolean not null default true;

create table public.invitations (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid references public.branches(id) on delete cascade,
  email text not null,
  full_name text not null,
  role_id integer not null references public.tbl_tipos_maestra(tima_id),
  status text not null default 'PENDIENTE',
  invited_by uuid references auth.users(id),
  created_at timestamptz not null default now(),
  accepted_at timestamptz,
  unique (tenant_id, email)
);
create index invitations_tenant_idx on public.invitations(tenant_id);

-- ============================================================
-- F0.3 Seguridad de login
-- ============================================================
create table public.login_attempts (
  id uuid primary key default gen_random_uuid(),
  email text not null,
  ip text,
  user_agent text,
  success boolean not null,
  reason text,
  at timestamptz not null default now()
);
create index login_attempts_email_idx on public.login_attempts(email, at desc);

-- ============================================================
-- F0.4 Ajustes por sucursal
-- ============================================================
create table public.branch_settings (
  branch_id uuid primary key references public.branches(id) on delete cascade,
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  data jsonb not null default '{}'::jsonb,
  updated_by uuid references auth.users(id),
  updated_at timestamptz not null default now()
);
create index branch_settings_tenant_idx on public.branch_settings(tenant_id);

-- ============================================================
-- F0.5 Auditoría inmutable
-- ============================================================
create table public.audit_log (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid references public.branches(id) on delete set null,
  user_id uuid,
  action text not null,
  entity text not null,
  entity_id uuid,
  before jsonb,
  after jsonb,
  ip text,
  at timestamptz not null default now()
);
create index audit_log_tenant_idx on public.audit_log(tenant_id, at desc);
create index audit_log_branch_idx on public.audit_log(branch_id, at desc);

-- Bloqueo inmutable de audit_log contra UPDATE y DELETE
create or replace function public.audit_log_immutable()
returns trigger
language plpgsql
as $$
begin
  raise exception 'audit_log is append-only: updates and deletes are forbidden';
end;
$$;

create trigger audit_log_immutable_trg
before update or delete on public.audit_log
for each row execute function public.audit_log_immutable();

-- ============================================================
-- F0.6 Numeración unificada
-- ============================================================
alter table public.document_sequences drop constraint if exists document_sequences_doc_type_check;
alter table public.document_sequences add constraint document_sequences_doc_type_check
  check (doc_type in ('POS', 'VENTA', 'DEVOLUCION', 'COMPRA', 'SEPARADO', 'COTIZACION', 'ACUERDO_PAGO', 'TRASLADO', 'GASTO', 'RUTA'));

-- ============================================================
-- F0.7 Outbox
-- ============================================================
create table public.outbox (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  kind text not null,
  payload jsonb not null,
  status text not null default 'PENDIENTE',
  attempts integer not null default 0,
  next_attempt_at timestamptz not null default now(),
  last_error text,
  created_at timestamptz not null default now()
);
create index outbox_status_idx on public.outbox(status, next_attempt_at);

-- ============================================================
-- RLS y Permisos
-- ============================================================
alter table public.permissions enable row level security;
alter table public.role_permissions enable row level security;
alter table public.tenant_role_overrides enable row level security;
alter table public.invitations enable row level security;
alter table public.login_attempts enable row level security;
alter table public.branch_settings enable row level security;
alter table public.audit_log enable row level security;
alter table public.outbox enable row level security;

create policy permissions_read on public.permissions for select using (true);
create policy role_permissions_read on public.role_permissions for select using (true);

create policy tenant_role_overrides_isolation on public.tenant_role_overrides for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

create policy invitations_isolation on public.invitations for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

create policy branch_settings_isolation on public.branch_settings for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

create policy audit_log_isolation on public.audit_log for select
  using (tenant_id = public.current_tenant_id());

create policy audit_log_insert on public.audit_log for insert
  with check (tenant_id = public.current_tenant_id());

create policy outbox_isolation on public.outbox for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

grant select on public.permissions, public.role_permissions to authenticated;
grant select, insert, update, delete on public.tenant_role_overrides, public.invitations, public.branch_settings, public.outbox to authenticated;
grant select, insert on public.audit_log to authenticated;

