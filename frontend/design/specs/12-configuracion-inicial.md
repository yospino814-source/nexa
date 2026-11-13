[Pegar antes el contenido de 00-design-system.md]

Construí la pantalla **"Configuración inicial"** (lo primero que ve el dueño después de crear la cuenta). Layout: **público**. Es un asistente de 3 pasos en una sola tarjeta centrada (máx. ~640px).

Arriba de la tarjeta: indicador de pasos horizontal (1 Tipo de negocio · 2 Sucursal · 3 Datos fiscales), con el paso actual en color primario y los completados con check. Abajo de cada paso: botón secundario "Atrás" a la izquierda y primario "Continuar" a la derecha.

**Paso 1 — ¿Qué tipo de negocio tenés?**
- Texto chico: "Podés elegir más de uno. Esto define qué módulos vas a ver."
- 4 tarjetas seleccionables en grilla 2x2, cada una con ícono, nombre y una sola línea de descripción:
  - Tienda / mostrador — "Venta rápida con lector de código de barras"
  - Restaurante / bar — "Mesas, comandas y pantalla de cocina"
  - Distribuidora / mayorista — "Pedidos por cliente y rutas de entrega"
  - Tienda virtual / domicilios — "Tus clientes piden desde una página web"
- Tarjeta seleccionada: borde en color primario + check en la esquina.

**Paso 2 — Tu primera sucursal**
- Campos: Nombre de la sucursal (placeholder "Tienda Centro"), Código corto (placeholder "CENTRO", opcional), Ciudad, Dirección.
- Zona horaria ya completada con "América/Bogotá" (editable, en gris).
- Solo si en el paso 1 eligió Restaurante: campo numérico "¿Cuántas mesas tenés?" con botones − / +.

**Paso 3 — Datos fiscales**
- Texto chico: "Los necesitás para facturar electrónicamente. Podés completarlos después."
- Campos: Razón social, NIT + DV en la misma fila (el DV es un campo chico de 1 dígito que se calcula solo y se muestra en gris).
- Link "Completar después" junto al botón principal.
- El botón principal de este paso dice "Terminar".

**Pantalla final — Todo listo**
- Ícono de check grande en verde, título "Tu negocio está listo", y una línea con lo que se configuró (ej. "Tienda / mostrador · Tienda Centro").
- Botón primario grande "Empezar a vender" y link secundario "Ir al panel".
