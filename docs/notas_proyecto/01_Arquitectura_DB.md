# Esquema Unificado de Base de Datos (POS Omnicanal)

Este documento contiene la versión final del diseño de la base de datos para PostgreSQL. Este esquema soporta múltiples flujos de negocio sin requerir tablas redundantes, utilizando relaciones opcionales (nulas).

## Flujos Soportados
1. **Tienda de Víveres (Retail):** Usa `ubicaciones` (tipo TIENDA), `ventas` (sin mesa ni ruta), `inventario_ubicaciones`.
2. **Restaurantes / Bares:** Usa `ubicaciones` (tipo RESTAURANTE), `mesas`. Las ventas se enlazan al `id_mesa`.
3. **Distribuidora Mayorista (Bodega):** Usa `ubicaciones` (tipo BODEGA), `rutas`, `clientes`. Las ventas se enlazan al `id_ruta` y `id_cliente`.

## Consideraciones de Diseño
- **Tabla Maestra:** Se utiliza una tabla jerárquica (Padre -> Hijo) para manejar todas las listas desplegables del sistema (Estados, Tipos de Documento, Métodos de Pago).
- **Inventario Separado:** Se eliminó el stock global. Ahora el inventario vive en `inventario_ubicaciones` (Producto + Sucursal).
- **Ventas Dinámicas:** La tabla `ventas` actúa como pivote. Contiene `id_mesa`, `id_ruta` e `id_cliente` como campos opcionales (`NULLABLE`).
- **Restricción de Integridad:** Un `CONSTRAINT CHECK` garantiza que una venta no pueda estar asignada a una mesa y a una ruta simultáneamente.

## Siguientes pasos posibles
- Integrar la lógica "Multi-Tenant" (SaaS) añadiendo la tabla `empresas` en la cima de la jerarquía.
- Definir el manejo de Recetas/Insumos para descontar ingredientes en restaurantes.
- Crear endpoints base en Rust.
