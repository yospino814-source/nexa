# 03 - Benchmarking Competitivo y Módulo de Compras Predictivas

Documento estratégico para el diseño del backend en **Rust (Axum)** de **Nexa POS**, integrando lo mejor de los sistemas analizados y el gran diferenciador de **Reabastecimiento Inteligente**.

---

## 1. Benchmarking de Competidores Analizados

| Sistema | Stack / Arquitectura | Fortalezas Clave | Falla Crítica | Qué adoptamos en Nexa |
| :--- | :--- | :--- | :--- | :--- |
| **Vendix Online** | Angular 20 + NestJS (Node) en AWS S3/CloudFront. 302 endpoints. | Multi-tenant por Host, escaneo de inventario físico con IA. | Sobrecargado ("Frankenstein"), 100% dependiente de internet, alertas de stock estáticas. | Modelo multi-tenant limpio y recepción de mercancía mediante escaneo inteligente. |
| **Loyverse** | Cloud + Apps móviles nativas. | La mejor velocidad de mostrador del mercado, cobro en 3 toques, modo Offline-First. | Módulo de compras prácticamente inexistente. | Diseño de checkout ultra rápido y soporte de ventas offline con sincronización. |
| **Alegra POS** | Cloud web app. | Conexión directa a balanzas digitales USB por navegador (Web Serial API). | Flujo de compras engorroso y tradicional. | Soporte nativo para venta por peso / balanzas electrónicas en mostrador. |
| **Bsale** | Cloud multi-sucursal. | Control quirúrgico de costos promedio y cálculo de márgenes netos reales por producto. | Alto costo y curva de configuración compleja. | Modelo de costeo riguroso (margen real = precio venta - costo de compra). |
| **Bind ERP** | ERP Cloud en México. | Gestión de cuentas por pagar a proveedores y fechas límite de crédito comercial. | Interfaz pesada, no diseñada para filas rápidas de retail. | Fechas de vencimiento y plazos de crédito de proveedores (15/30/60 días). |

---

## 2. El Diferenciador Central de Nexa: Smart Restock (Compras Predictivas)

La mayoría de los POS solo avisan cuando el stock toca un "mínimo estático" (ej. 5 unidades). Si un producto rota a 30 unidades diarias y el proveedor tarda 3 días, la tienda quiebra stock 2 días antes.

### Algoritmo de Demanda y Reorden Dinámico
1. **Velocidad de Venta Diaria ($V_d$):**
   $$V_d = \frac{\sum \text{Unidades vendidas en los últimos 15 días}}{15}$$

2. **Días de Cobertura de Stock ($D_c$):**
   $$D_c = \frac{\text{Stock actual en tienda}}{V_d}$$

3. **Punto de Reorden ($ROP$):**
   $$ROP = (V_d \times \text{Días de entrega del proveedor}) + \text{Stock de seguridad}$$

4. **Disparador:**
   Si $\text{Stock actual} \le ROP$, el producto se añade automáticamente a la **Sugerencia de Pedido**.

---

## 3. Modelo de Base de Datos para Compras y Proveedores (PostgreSQL / Supabase)

Para soportar este módulo en el esquema omnicanal existente de Nexa:

```sql
-- Catálogo de Proveedores
CREATE TABLE proveedores (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    empresa_id UUID NOT NULL REFERENCES empresas(id),
    nombre VARCHAR(150) NOT NULL,
    contacto_nombre VARCHAR(100),
    telefono_whatsapp VARCHAR(20) NOT NULL,
    email VARCHAR(100),
    dias_credito INT DEFAULT 0, -- 0 = Contado, 15, 30, etc.
    dias_entrega_promedio INT DEFAULT 2, -- Lead time
    activo BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Relación Producto - Proveedor (Costos y códigos)
CREATE TABLE proveedor_productos (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    proveedor_id UUID NOT NULL REFERENCES proveedores(id),
    producto_id UUID NOT NULL REFERENCES productos(id),
    codigo_proveedor VARCHAR(50),
    costo_compra DECIMAL(12, 2) NOT NULL,
    unidad_compra VARCHAR(20) DEFAULT 'UNIDAD', -- CAJA, BULTO, DOCENA, UNIDAD
    factor_conversion INT DEFAULT 1, -- Si 1 caja = 24 unidades
    es_principal BOOLEAN DEFAULT TRUE
);

-- Órdenes de Compra Sugeridas y Reales
CREATE TABLE ordenes_compra (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    empresa_id UUID NOT NULL REFERENCES empresas(id),
    proveedor_id UUID NOT NULL REFERENCES proveedores(id),
    ubicacion_destino_id UUID NOT NULL REFERENCES ubicaciones(id),
    numero_orden VARCHAR(30) UNIQUE NOT NULL,
    estado VARCHAR(20) DEFAULT 'SUGERIDO', -- SUGERIDO, BORRADOR, ENVIADO_WHATSAPP, CONFIRMADO, RECIBIDO, CANCELADO
    total_estimado DECIMAL(12, 2) NOT NULL DEFAULT 0.00,
    fecha_limite_pago DATE,
    mensaje_whatsapp_generado TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE ordenes_compra_detalles (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    orden_compra_id UUID NOT NULL REFERENCES ordenes_compra(id) ON DELETE CASCADE,
    producto_id UUID NOT NULL REFERENCES productos(id),
    cantidad_sugerida DECIMAL(10, 2) NOT NULL,
    cantidad_confirmada DECIMAL(10, 2),
    cantidad_recibida DECIMAL(10, 2) DEFAULT 0,
    costo_unitario DECIMAL(12, 2) NOT NULL,
    subtotal DECIMAL(12, 2) NOT NULL
);
```

---

## 4. Endpoints Propuestos para el Backend en Rust (Axum)

1. `GET /api/v1/suppliers`: Listado de distribuidores y proveedores.
2. `POST /api/v1/suppliers`: Crear nuevo proveedor.
3. `GET /api/v1/inventory/reorder-suggestions`: **El endpoint cerebro**. Calcula los productos cuya cobertura es menor al tiempo de entrega del distribuidor.
4. `POST /api/v1/purchase-orders/generate-suggested`: Agrupa las alertas por proveedor y crea borradores de órdenes de compra con 1 solo clic.
5. `GET /api/v1/purchase-orders/:id/whatsapp-payload`: Genera el texto estructurado listo para abrir la API de `https://wa.me/{telefono}?text=...` con el pedido formalizado.
6. `POST /api/v1/purchase-orders/:id/receive`: Recepción de mercancía que impacta `inventario_ubicaciones` y actualiza los costos promedio.
