# La Tabla Maestra: Diccionario de Datos Dinámico

En el diseño de arquitectura de bases de datos, especialmente en sistemas ERP y POS (como ChefControl), nos encontramos con decenas de atributos que son básicamente listas desplegables estáticas o catálogos (ej: "Tipos de Documento", "Estados de Pedido", "Unidades de Medida", "Roles de Usuario").

Crear una tabla independiente para cada una de estas listas (`tbl_estados_venta`, `tbl_tipos_documento`, `tbl_roles`) inflaría la base de datos con mini-tablas de 3 a 5 registros, complicando el diagrama y el mantenimiento.

La solución es el patrón **"One True Lookup Table" (OTLT) o Tabla Maestra de Parámetros**.

## ¿Qué es y cómo funciona?
Es una única tabla fuertemente tipada que almacena múltiples catálogos de datos mediante una jerarquía recursiva (Padre -> Hijo).

### Estructura Base:
- `tima_id`: El identificador único del parámetro.
- `padre_id`: Si es nulo (NULL), indica que el registro es una **cabecera o categoría raíz** (Ej: "ESTADOS DE MESA"). Si tiene un número, es una **opción hija** que pertenece a un padre (Ej: "LIBRE", que apunta al ID de "ESTADOS DE MESA").
- `tima_codigo`: Un identificador corto (Ej: 'CC', 'NIT', 'PAG').
- `tima_nombre`: El valor legible para el usuario.

## Usos y Aplicaciones
1. **Centralización de Catálogos:** Un solo endpoint en el backend puede devolver cualquier lista desplegable al frontend simplemente filtrando por el código del padre.
2. **Escalabilidad Sin Migraciones:** Si mañana la DIAN exige un nuevo tipo de factura, o el restaurante quiere aceptar "Criptomonedas" como medio de pago, el administrador del sistema simplemente inserta un registro nuevo en esta tabla. NO se requiere que un desarrollador escriba migraciones SQL (`CREATE TABLE` o `ALTER TABLE`).
3. **Optimización de Joins:** Al usar una sola tabla para el 80% de los metadatos del sistema, el motor de PostgreSQL puede cachearla fácilmente en RAM.

## Aplicación en el Contexto de ChefControl
En este proyecto, se aplicó una regla estricta: **Cero sobre-diseño**.
Todas las tablas operativas (`users`, `orders`, `products`, `kds_stations`) utilizan llaves foráneas (`INT`) que apuntan directamente a `tbl_tipos_maestra(tima_id)`.

*Ejemplo en el flujo Multirubro:*
- La tabla `orders` tiene un `sale_type_id`. Este ID apunta a un hijo en la tabla maestra cuyo padre es "MODALIDADES DE VENTA". Los hijos son: 'MESA', 'MOSTRADOR', 'MAYORISTA', 'DOMICILIO'.
- De esta manera, el mismo backend y la misma base de datos soportan el flujo de un Restaurante, una Licorera y una Distribuidora, simplemente evaluando el `sale_type_id` de la tabla maestra.

Para facilitar la vida de los desarrolladores del frontend y de reportes, se creó una vista SQL (`vw_catalogo_maestro`) que aplana esta recursividad y entrega el nombre del parámetro junto al código de su padre en una sola fila.
