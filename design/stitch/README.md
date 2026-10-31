# Nexa POS - Suite Completa de Módulos UI & Arquitectura

Bienvenido al paquete completo de módulos y pantallas para **Nexa POS**, el sistema de punto de venta y gestión omnicanal para el mercado colombiano (retail, gastronomía, distribución mayorista y e-commerce).

---

## 🚀 Inicio Rápido: Explorador Visual Interactivo

Para explorar todas las pantallas de forma interactiva:
1. Abre el archivo **`index.html`** en cualquier navegador web (doble clic).
2. Podrás:
   - Filtrar por cualquiera de los **7 módulos funcionales**.
   - Ver miniaturas de las **30 pantallas**.
   - Abrir cada pantalla a pantalla completa con su código HTML/Tailwind nativo.
   - Probar el **simulador responsivo** (Desktop 100%, Tablet 768px, Móvil 390px) en un visor modal integrado.
   - Ver y descargar las capturas en alta definición (`.png`).

---

## 📁 Estructura del Paquete

```text
nexa-pos-modulos/
│
├── index.html                           <- Visor / Portal interactivo de todas las pantallas
├── manifest.json                        <- Manifiesto técnico con metadatos de las 30 pantallas
├── README.md                            <- Esta guía de uso y arquitectura
│
├── 01_Operacion_POS_Caja/               <- Módulo 1: POS Mostrador, Cobro Rápido y Arqueo Ciego
│   ├── 13-pos-mostrador.html / .png
│   ├── 14-cobro-y-comprobante.html / .png
│   └── 17-caja.html / .png
│
├── 02_Restaurante_Gastronomia/          <- Módulo 2: Salón, Mesas, Comandas, KDS y Escandallo
│   ├── 21-mapa-de-mesas.html / .png
│   ├── 22-comanda.html / .png
│   ├── 23-cocina-kds.html / .png
│   ├── 24-cuenta-de-mesa.html / .png
│   └── 25-recetas-e-insumos.html / .png
│
├── 03_Inventario_Compras/               <- Módulo 3: Stock Info, Compras Scan IA, Ficha y Kardex
│   ├── 05-stock-info.html / .png
│   ├── 08-compras.html / .png
│   ├── 15-ficha-de-producto.html / .png
│   └── 16-movimientos-de-inventario.html / .png
│
├── 04_Ventas_Finanzas/                  <- Módulo 4: Dashboard, Ventas por Canal, Rentabilidad y Gastos
│   ├── 01-panel-principal.html / .png
│   ├── 02-resumen-de-ventas.html / .png
│   ├── 03-ventas-por-producto.html / .png
│   ├── 04-ordenes.html / .png
│   └── 06-gastos.html / .png
│
├── 05_Clientes_Mayorista_B2B/           <- Módulo 5: Directorio, Fiado, Rutas y Preventa B2B
│   ├── 07-clientes.html / .png
│   ├── 26-rutas.html / .png
│   ├── 27-pedido-mayorista.html / .png
│   └── 28-app-del-repartidor.html / .png
│
├── 06_Ecommerce_Publico/                <- Módulo 6: Tienda Virtual, Checkout y Tracking
│   ├── 29-catalogo-publico.html / .png
│   ├── 30-carrito-y-checkout.html / .png
│   └── 31-seguimiento-del-pedido.html / .png
│
├── 07_Configuracion_DIAN/               <- Módulo 7: Registro, Onboarding, Sedes, Roles y DIAN
│   ├── 09-crear-cuenta.html / .png
│   ├── 11-iniciar-sesion.html / .png
│   ├── 12-configuracion-inicial.html / .png
│   ├── 18-negocio-y-sucursales.html / .png
│   ├── 19-usuarios-y-roles.html / .png
│   └── 20-pagos-impuestos-y-facturacion.html / .png
│
├── especificaciones_prompts/           <- 32 archivos Markdown con las especificaciones de diseño
│   ├── 00-design-system.md
│   └── ... (01 al 31)
│
└── documentacion_backend/               <- Contexto técnico, arquitectura DB y algoritmos
    ├── 01_Arquitectura_DB.md
    ├── 02_Tabla_Maestra_Diccionario_Datos.md
    ├── 03_Benchmarking_y_Modulo_Compras_Inteligentes.md
    ├── 04_Catalogo_Pantallas_Stitch_Nexa_UI.md
    └── Index.md
```

---

## 🎨 Sistema de Diseño (Design System)

- **ID del Proyecto Stitch**: `projects/11495705857039807858`
- **Design System Vinculado**: `assets/16189402570785253940`
- **Paleta de Color**:
  - Primario de Marca: Azul Petróleo `#0F6E6E` (`teal-700` / `cyan-800`).
  - Fondo de Aplicación: `slate-50`.
  - Tarjetas y Contenedores: Blanco con bordes `rounded-2xl` y borde sutil `slate-200/60`.
  - Navegación Administrativa: Barra lateral de 72px en tono oscuro `slate-900`.
  - Semántica: Esmeralda `emerald-500` (positivo/activo), Ámbar `amber-500` (alerta de stock/límites), Rojo `rose-500` (agotado/crítico).
- **Tipografía**: `Inter` (Google Fonts) en toda la aplicación.
- **Formato Monetario**: Pesos Colombianos COP (`$XX.XXX`, sin decimales, con punto separador de miles).

---

## 🛠️ Conexión con el Backend (`nexa-backend`)

El proyecto está diseñado para conectarse fluidamente con el backend en Rust (`axum 0.8` + `sqlx` + PostgreSQL/Supabase):
1. **Algoritmo de Compras Inteligentes**: Cálculo automático del Punto de Reorden ($ROP = V_d \times LT + SS$) documentado en `03_Benchmarking_y_Modulo_Compras_Inteligentes.md`.
2. **Esquema Omnicanal Unificado**: Tablas para Retail (código de barras, kardex), Gastronomía (mesas, comandas, KDS, escandallo por gramo/ml), y Logística Mayorista (preventa B2B con validación en vivo de cupo de crédito).
