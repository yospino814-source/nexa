-- Datos del licor en el producto: grados de alcohol y contenido de la botella.
-- Sirven para el catálogo y, más adelante, para el impuesto al consumo de
-- licores (ICL) si un contador confirma que el tendero debe discriminarlo.

alter table public.products
  add column if not exists alcohol_degrees numeric(5,2) check (alcohol_degrees is null or (alcohol_degrees >= 0 and alcohol_degrees <= 100)),
  add column if not exists volume_ml numeric(10,2) check (volume_ml is null or volume_ml > 0);
