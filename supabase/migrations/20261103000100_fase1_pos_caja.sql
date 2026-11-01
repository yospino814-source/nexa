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
