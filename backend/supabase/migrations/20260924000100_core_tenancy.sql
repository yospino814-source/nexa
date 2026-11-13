-- Capa 0 - Tipos maestros, tenancy, identidad y RLS

-- ============================================================
-- Tabla maestra recursiva de tipos y estados
-- ============================================================
create table public.tbl_tipos_maestra (
  tima_id     integer generated always as identity primary key,
  tima_nombre varchar(60) not null,
  tima_codigo varchar(30),
  padre_id    integer references public.tbl_tipos_maestra(tima_id) on delete restrict,
  constraint uq_padre_codigo unique (padre_id, tima_codigo)
);
create unique index tbl_tipos_maestra_header_idx
  on public.tbl_tipos_maestra (tima_codigo) where padre_id is null;
create index tbl_tipos_maestra_padre_idx on public.tbl_tipos_maestra (padre_id);

-- Cabeceras (padre_id null)
insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id) values
  ('TIPOS DE ROL', 'ROL', null),
  ('TIPOS DE UNIDAD DE MEDIDA', 'UOM', null),
  ('TIPOS DE IMPUESTO', 'IMP', null),
  ('TIPOS DE ESTADO DE ORDEN', 'ORD', null),
  ('TIPOS DE ESTADO DE MESA', 'MESA', null),
  ('TIPOS DE ESTADO DE RESERVA', 'RES', null),
  ('TIPOS DE MOVIMIENTO DE INVENTARIO', 'MOV', null),
  ('TIPOS DE VENTA', 'VENTA', null),
  ('TIPOS DE ESTADO DE ITEM', 'ITEM', null);

-- Hijos (opciones utilizables)
insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.nombre, c.codigo, h.tima_id
from (values ('Mesero','MESERO'),('Cocina','COCINA'),('Admin/Caja','ADMIN_CAJA')) c(nombre, codigo)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL') h;

insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.nombre, c.codigo, h.tima_id
from (values ('Unidad','UND'),('Gramo','G'),('Mililitro','ML')) c(nombre, codigo)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'UOM') h;

insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.nombre, c.codigo, h.tima_id
from (values ('IVA 19%','IVA19'),('IVA 5%','IVA5'),('INC 8%','INC8')) c(nombre, codigo)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'IMP') h;

insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.nombre, c.codigo, h.tima_id
from (values ('Creada','CREADA'),('En preparacion','EN_PREPARACION'),('Lista','LISTA'),
             ('Entregada','ENTREGADA'),('Cobrada','COBRADA'),('Cerrada','CERRADA'),
             ('Cancelada','CANCELADA')) c(nombre, codigo)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ORD') h;

insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.nombre, c.codigo, h.tima_id
from (values ('Libre','LIBRE'),('Ocupada','OCUPADA')) c(nombre, codigo)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'MESA') h;

insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.nombre, c.codigo, h.tima_id
from (values ('Activa','ACTIVA'),('Confirmada','CONFIRMADA'),('Liberada','LIBERADA'),
             ('Expirada','EXPIRADA')) c(nombre, codigo)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'RES') h;

insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.nombre, c.codigo, h.tima_id
from (values ('Descuento','DESCUENTO'),('Reposicion','REPOSICION'),('Ajuste','AJUSTE')) c(nombre, codigo)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'MOV') h;

insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.nombre, c.codigo, h.tima_id
from (values ('Mesa','MESA'),('Mostrador','MOSTRADOR'),('Domicilio','DOMICILIO'),
             ('Mayorista','MAYORISTA')) c(nombre, codigo)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'VENTA') h;

insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.nombre, c.codigo, h.tima_id
from (values ('Pendiente','PENDIENTE'),('En preparacion','EN_PREPARACION'),('Listo','LISTO')) c(nombre, codigo)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'ITEM') h;

-- Resolucion de un hijo por (cabecera, codigo)
create or replace function public.tipo_id(header text, child text)
returns integer
language sql
stable
set search_path = ''
as $$
  select c.tima_id
    from public.tbl_tipos_maestra c
    join public.tbl_tipos_maestra h on h.tima_id = c.padre_id
   where h.padre_id is null and h.tima_codigo = header and c.tima_codigo = child
$$;

-- ============================================================
-- Tenants, sucursales, usuarios y membresias por sucursal
-- ============================================================
create table public.tenants (
  id uuid primary key default gen_random_uuid(),
  name text not null,
  nit text,
  dv char(1),
  business_name text,
  created_at timestamptz not null default now()
);

create table public.branches (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  code text,
  name text not null,
  timezone text not null default 'America/Bogota',
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  unique (tenant_id, code)
);
create index branches_tenant_idx on public.branches (tenant_id);

create table public.users (
  id uuid primary key references auth.users(id) on delete cascade,
  full_name text not null,
  created_at timestamptz not null default now()
);

create table public.memberships (
  id uuid primary key default gen_random_uuid(),
  user_id uuid not null references public.users(id) on delete cascade,
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid,
  role_id integer not null references public.tbl_tipos_maestra(tima_id),
  created_at timestamptz not null default now(),
  unique nulls not distinct (user_id, tenant_id, branch_id),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);
create index memberships_user_idx on public.memberships (user_id);
create index memberships_tenant_idx on public.memberships (tenant_id);

create table public.user_active_context (
  user_id uuid primary key references auth.users(id) on delete cascade,
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid,
  updated_at timestamptz not null default now(),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id)
);

-- ============================================================
-- Helpers de contexto (fail-closed sin GUC)
-- ============================================================
create or replace function public.current_tenant_id()
returns uuid
language sql
stable
set search_path = ''
as $$
  select nullif(current_setting('app.current_tenant_id', true), '')::uuid
$$;

create or replace function public.current_branch_id()
returns uuid
language sql
stable
set search_path = ''
as $$
  select nullif(current_setting('app.current_branch_id', true), '')::uuid
$$;

-- Predicado de aislamiento tenant + sucursal reutilizable
create or replace function public.in_scope(t uuid, b uuid)
returns boolean
language sql
stable
set search_path = ''
as $$
  select t = public.current_tenant_id()
     and (public.current_branch_id() is null or b = public.current_branch_id())
$$;

-- ============================================================
-- Row-Level Security
-- ============================================================
alter table public.tbl_tipos_maestra enable row level security;
alter table public.tenants enable row level security;
alter table public.branches enable row level security;
alter table public.users enable row level security;
alter table public.memberships enable row level security;
alter table public.user_active_context enable row level security;

create policy tbl_tipos_maestra_read on public.tbl_tipos_maestra
  for select using (true);

create policy tenants_member_read on public.tenants
  for select using (
    id in (select m.tenant_id from public.memberships m where m.user_id = auth.uid())
  );

create policy branches_isolation on public.branches for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

create policy users_self on public.users for all
  using (id = auth.uid()) with check (id = auth.uid());

create policy memberships_self_read on public.memberships
  for select using (user_id = auth.uid());

create policy user_active_context_self on public.user_active_context for all
  using (user_id = auth.uid()) with check (user_id = auth.uid());

-- ============================================================
-- Custom access token hook (tenant + sucursal activos)
-- ============================================================
create or replace function public.custom_access_token_hook(event jsonb)
returns jsonb
language plpgsql
stable
security definer
set search_path = ''
as $$
declare
  claims jsonb;
  ctx record;
begin
  claims := event -> 'claims';
  select uc.tenant_id, uc.branch_id into ctx
    from public.user_active_context uc
   where uc.user_id = (event ->> 'user_id')::uuid;
  if ctx.tenant_id is not null then
    claims := jsonb_set(claims, '{app_metadata,tenant_id}', to_jsonb(ctx.tenant_id::text));
    if ctx.branch_id is not null then
      claims := jsonb_set(claims, '{app_metadata,branch_id}', to_jsonb(ctx.branch_id::text));
    end if;
  end if;
  return jsonb_set(event, '{claims}', claims);
end;
$$;

-- ============================================================
-- Grants
-- ============================================================
grant usage on schema public to authenticated;
grant select on public.tbl_tipos_maestra to authenticated;
grant select, insert, update, delete on public.tenants, public.branches, public.users, public.memberships, public.user_active_context to authenticated;
grant execute on function public.current_tenant_id() to authenticated;
grant execute on function public.current_branch_id() to authenticated;
grant execute on function public.in_scope(uuid, uuid) to authenticated;
grant execute on function public.tipo_id(text, text) to authenticated;

grant select on public.user_active_context to supabase_auth_admin;
grant execute on function public.custom_access_token_hook to supabase_auth_admin;
revoke execute on function public.custom_access_token_hook from authenticated, anon, public;
