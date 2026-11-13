-- Capa 3/2 - Impuesto por linea y ajustes de inventario con direccion

-- ============================================================
-- Impuesto snapshot por linea de orden
-- ============================================================
-- La tasa se congela al agregar la linea (igual que unit_price), de modo que
-- un cambio posterior del impuesto del producto no altera ordenes existentes.
-- Convencion: el impuesto se suma sobre el precio (precio sin impuesto).
alter table public.order_items
  add column tax_rate numeric(5,4) not null default 0 check (tax_rate >= 0),
  add column tax_amount numeric(14,2) generated always as (qty * unit_price * tax_rate) stored;

-- ============================================================
-- Ajustes de inventario con direccion explicita
-- ============================================================
-- `AJUSTE` se conserva por compatibilidad con movimientos ya registrados,
-- pero los ajustes nuevos usan AJUSTE_ENTRADA / AJUSTE_SALIDA para que el
-- kardex sepa si el ajuste sumo o resto unidades.
insert into public.tbl_tipos_maestra (tima_nombre, tima_codigo, padre_id)
select c.nombre, c.codigo, h.tima_id
from (values ('Ajuste de entrada','AJUSTE_ENTRADA'),('Ajuste de salida','AJUSTE_SALIDA')) c(nombre, codigo)
cross join (select tima_id from public.tbl_tipos_maestra where padre_id is null and tima_codigo = 'MOV') h;
