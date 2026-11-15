-- Facturación electrónica: documento equivalente POS electrónico y notas
-- crédito POS a través de un proveedor tecnológico (MATIAS API por ahora).
-- La venta ya deja su documento en `fiscal_documents` como PENDIENTE; un
-- proceso del backend lo envía, guarda el CUDE, el QR y el PDF.

-- Configuración por sede.
create table public.fiscal_settings (
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  branch_id uuid not null,
  -- SIMULADO: no sale a internet, inventa un CUDE (desarrollo y demos).
  -- MATIAS: envía a MATIAS API (sandbox o producción según el backend).
  provider text not null default 'SIMULADO' check (provider in ('SIMULADO', 'MATIAS')),
  enabled boolean not null default false,
  -- Casa de Software: sub-cuenta del negocio en el proveedor (opcional).
  provider_client_id text,
  -- Dirección que sale en el documento POS (punto de venta).
  pos_address text,
  updated_at timestamptz not null default now(),
  primary key (branch_id),
  foreign key (tenant_id, branch_id) references public.branches (tenant_id, id) on delete cascade
);
alter table public.fiscal_settings enable row level security;
create policy fiscal_settings_isolation on public.fiscal_settings for all to authenticated
  using (public.in_scope(tenant_id, branch_id)) with check (public.in_scope(tenant_id, branch_id));
grant select, insert, update, delete on public.fiscal_settings to authenticated;

-- Resultado del envío.
alter table public.fiscal_documents
  add column provider text,
  add column provider_uuid text,
  add column qr_url text,
  add column pdf_url text,
  add column sent_at timestamptz,
  add column accepted_at timestamptz;

-- Estado "en proceso" de la DIAN: se vuelve a consultar más tarde.
insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select 'En proceso', 'EN_PROCESO', h.tima_id from public.tbl_tipos_maestra h
where h.padre_id is null and h.tima_codigo = 'DOCF'
on conflict (padre_id, tima_codigo) do nothing;

-- El proceso de envío no tiene usuario: esta función le entrega los
-- documentos que tocan, de todos los negocios, y los aparta por 2 minutos
-- para que dos copias del backend no envíen el mismo.
create or replace function private.fiscal_claim_due(p_limit integer)
returns table (id uuid, tenant_id uuid, branch_id uuid)
language sql
volatile
security definer
set search_path = ''
as $$
  with due as (
    select f.id
      from public.fiscal_documents f
      join public.fiscal_settings s on s.branch_id = f.branch_id and s.enabled
     where f.status_id in (public.tipo_id('DOCF', 'PENDIENTE'), public.tipo_id('DOCF', 'EN_PROCESO'))
       and f.next_attempt_at <= now()
       and f.attempts < 12
     order by f.next_attempt_at
     limit greatest(1, least(p_limit, 50))
     for update of f skip locked
  )
  update public.fiscal_documents f
     set next_attempt_at = now() + interval '2 minutes'
    from due
   where f.id = due.id
  returning f.id, f.tenant_id, f.branch_id
$$;
revoke all on function private.fiscal_claim_due(integer) from public, anon;
grant execute on function private.fiscal_claim_due(integer) to authenticated;
