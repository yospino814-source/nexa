-- Impuesto al Consumo de Licores (ICL) y datos alcoholimetricos del producto
-- Ley 1816 de 2016 y tributo DIAN 32 (tax_id 18 en MATIAS API).

alter table public.products
  add column if not exists alcohol_degrees numeric(5,2) check (alcohol_degrees is null or (alcohol_degrees >= 0 and alcohol_degrees <= 100)),
  add column if not exists volume_ml numeric(10,2) check (volume_ml is null or volume_ml > 0);

insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select 'ICL Licores', 'ICL', h.tima_id
from (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'IMP') h
where not exists (
  select 1 from public.tbl_tipos_maestra
  where tima_codigo = 'ICL' and padre_id = h.tima_id
);
