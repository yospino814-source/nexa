-- Capa 0 - Tenancy, identidad y RLS

create table public.tenants (
  id uuid primary key default gen_random_uuid(),
  name text not null,
  created_at timestamptz not null default now()
);

create table public.locations (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  code text,
  name text not null,
  timezone text not null default 'America/Bogota',
  created_at timestamptz not null default now(),
  unique (tenant_id, id),
  unique (tenant_id, code)
);
create index locations_tenant_idx on public.locations (tenant_id);

create table public.memberships (
  id uuid primary key default gen_random_uuid(),
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  user_id uuid not null references auth.users(id) on delete cascade,
  role text not null check (role in ('mesero','cocina','admin_caja')),
  created_at timestamptz not null default now(),
  unique (tenant_id, user_id)
);
create index memberships_user_idx on public.memberships (user_id);
create index memberships_tenant_idx on public.memberships (tenant_id);

create table public.user_active_tenant (
  user_id uuid primary key references auth.users(id) on delete cascade,
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  updated_at timestamptz not null default now()
);

create or replace function public.current_tenant_id()
returns uuid
language sql
stable
set search_path = ''
as $$
  select nullif(current_setting('app.current_tenant_id', true), '')::uuid
$$;

-- Row-Level Security
alter table public.tenants enable row level security;
alter table public.locations enable row level security;
alter table public.memberships enable row level security;
alter table public.user_active_tenant enable row level security;

create policy tenants_member_read on public.tenants
  for select using (
    id in (select m.tenant_id from public.memberships m where m.user_id = auth.uid())
  );

create policy memberships_self_read on public.memberships
  for select using (user_id = auth.uid());

create policy user_active_tenant_self on public.user_active_tenant
  for all using (user_id = auth.uid()) with check (user_id = auth.uid());

create policy locations_tenant_isolation on public.locations
  for all using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

-- Grants
grant select, insert, update, delete on public.tenants, public.locations, public.memberships, public.user_active_tenant to authenticated;
grant usage on schema public to authenticated;
grant execute on function public.current_tenant_id() to authenticated;
