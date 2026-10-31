[Pegar antes el contenido de 00-design-system.md]

Construí la pantalla **"Configuración · Pagos, impuestos y facturación"**. Layout: **administración**. Tres pestañas: "Métodos de pago", "Impuestos" y "Facturación electrónica".

**Pestaña Métodos de pago**
- Lista de filas, cada una con ícono, nombre, una línea gris de descripción e interruptor de activo:
  - Efectivo — "Con cálculo de cambio"
  - Tarjeta débito / crédito — "Con datáfono propio"
  - Transferencia — "Nequi, Daviplata o cuenta bancaria"
  - Crédito / fiado — "Solo para clientes con cupo asignado"
  - Pago en línea — "Para la tienda virtual" con insignia gris "Próximamente"
- Efectivo no se puede desactivar (interruptor bloqueado con tooltip).

**Pestaña Impuestos**
- Tabla de 4 columnas: Impuesto · Tarifa · Productos que lo usan · Predeterminado (radio).
- Filas: IVA 19%, IVA 5%, INC 8% (impuesto al consumo, para restaurantes), Exento.
- Línea gris de ayuda debajo: "El predeterminado se aplica a los productos nuevos".

**Pestaña Facturación electrónica**
1. Tarjeta de estado arriba: ícono + "No conectado" (gris) o "Conectado" (verde), con el nombre del proveedor tecnológico autorizado por la DIAN.
2. Formulario de conexión: Proveedor (dropdown con placeholder "Elegí tu proveedor autorizado"), Usuario o identificador de la cuenta, Clave de acceso (campo enmascarado con ícono de ojo), y botón secundario "Probar conexión" que muestra un resultado en línea (check verde o error rojo corto).
3. Tarjeta "Resolución de facturación": Prefijo, Número de resolución, Rango autorizado (desde – hasta), Vigencia (fecha hasta). Debajo, barra de progreso del consecutivo: "1.240 de 5.000 facturas usadas", en ámbar con aviso cuando pase el 80% o falten menos de 30 días para vencer.
4. Tarjeta "Tirilla POS": texto del encabezado, texto del pie de página e interruptor "Mostrar logo", con una vista previa chica de la tirilla al lado.

Datos mock: proveedor no conectado; resolución con prefijo "SETP", rango 1 a 5.000, 1.240 usadas, vigente hasta dentro de 8 meses.
