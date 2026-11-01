-- ============================================================
-- F2.1 Campos de producto y unidades
-- ============================================================

insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.n, c.c, h.tima_id from (values ('Kilogramo','KG'),('Litro','L'),('Metro','M'),
  ('Caja','CAJA'),('Paquete','PAQUETE'),('Bulto','BULTO'),('Libra','LB')) c(n, c)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'UOM') h
on conflict (padre_id, tima_codigo) do nothing;

alter table public.products
  add column kind text not null default 'FISICO' check (kind in ('FISICO','SERVICIO','PREPARADO')),
  add column usage text not null default 'VENTA' check (usage in ('VENTA','INSUMO','INTERNO')),
  add column purchase_uom_id integer references public.tbl_tipos_maestra(tima_id),
  add column purchase_factor numeric(14,4) not null default 1 check (purchase_factor > 0),
  add column reorder_qty numeric(14,4) check (reorder_qty > 0),
  add column max_qty numeric(14,4) check (max_qty > 0),
  add column track_lots boolean not null default false,
  add column parent_id uuid,
  add column has_variants boolean not null default false,
  add column variant_attrs jsonb,
  add column archived_at timestamptz,
  add column description text check (length(description) <= 500),
  add constraint products_parent_fkey foreign key (tenant_id, parent_id) references public.products (tenant_id, id),
  add constraint products_variant_shape check (
    (parent_id is null) or (has_variants = false and variant_attrs is not null)),
  add constraint products_service_no_lots check (kind <> 'SERVICIO' or track_lots = false);
