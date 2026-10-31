-- Capa 4 - Flujo de compras y proveedores a nivel de negocio

-- 1. Proveedores a nivel de todo el tenant: branch_id pasa a ser opcional
alter table public.suppliers alter column branch_id drop not null;

-- Unicidad compuesta para FKs que no usan branch_id
alter table public.suppliers
  add constraint suppliers_tenant_id_id_key unique (tenant_id, id);

-- NIT unico por negocio cuando viene especificado
create unique index if not exists suppliers_tenant_tax_id_idx
  on public.suppliers (tenant_id, tax_id)
  where tax_id is not null;

-- RLS de proveedores a nivel de tenant (no atada a sucursal)
drop policy if exists suppliers_isolation on public.suppliers;
create policy suppliers_isolation on public.suppliers for all
  using (tenant_id = public.current_tenant_id())
  with check (tenant_id = public.current_tenant_id());

-- 2. En compras, FK directa hacia suppliers(tenant_id, id)
alter table public.purchase_receipts
  drop constraint if exists purchase_receipts_tenant_id_branch_id_supplier_id_fkey;

alter table public.purchase_receipts
  add constraint purchase_receipts_tenant_supplier_fkey
  foreign key (tenant_id, supplier_id) references public.suppliers (tenant_id, id);

-- Fecha de la factura del proveedor y cuerpo original para idempotencia
alter table public.purchase_receipts
  add column if not exists supplier_invoice_date date,
  add column if not exists client_request jsonb;

-- 3. Estado PENDIENTE en la tabla maestra para compras
insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select 'Pendiente de revisión', 'PENDIENTE', h.tima_id
from public.tbl_tipos_maestra h
where h.padre_id is null and h.tima_codigo = 'PUR'
on conflict (padre_id, tima_codigo) do nothing;
