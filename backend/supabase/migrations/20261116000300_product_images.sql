-- Foto de cada producto (POS, catálogo del bot y ficha). Se guarda ya
-- comprimida por el navegador (webp/jpeg/png, máximo 512 KB) para no depender
-- de un servicio externo de archivos.

create table if not exists public.product_images (
  product_id uuid primary key references public.products(id) on delete cascade,
  tenant_id uuid not null references public.tenants(id) on delete cascade,
  mime text not null check (mime in ('image/webp', 'image/jpeg', 'image/png')),
  data bytea not null check (octet_length(data) between 1 and 524288),
  updated_at timestamptz not null default now()
);

alter table public.product_images enable row level security;
create policy product_images_isolation on public.product_images for all to authenticated
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());
grant select, insert, update, delete on public.product_images to authenticated;

-- Lectura pública por id (las etiquetas <img> no mandan el token). El id es un
-- UUID que solo aparece en el catálogo del negocio.
create or replace function private.product_image(p_product_id uuid)
returns table (mime text, data bytea, updated_at timestamptz)
language sql
stable
security definer
set search_path = ''
as $$
  select i.mime, i.data, i.updated_at from public.product_images i where i.product_id = p_product_id
$$;

revoke all on function private.product_image(uuid) from public, anon;
grant execute on function private.product_image(uuid) to authenticated;
