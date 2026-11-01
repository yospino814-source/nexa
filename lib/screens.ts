// Registro maestro de las 30 pantallas de Nexa POS.
//
// Cada pantalla tiene una ruta fija en la app. Mientras no se migre a React
// (`status: "diseno"`), la ruta muestra el HTML de Stitch de `design/stitch/`
// a traves de `app/(referencia)/[...slug]`. Para migrarla: crear la pagina real
// en el grupo de layout que corresponda y cambiar `status` a "react".

export type ScreenLayout = "admin" | "operacion" | "publico";
export type ScreenStatus = "react" | "diseno";

export type ModuleId =
  | "operacion"
  | "restaurante"
  | "inventario"
  | "ventas"
  | "clientes"
  | "ecommerce"
  | "configuracion"
  | "marketing";

export type Screen = {
  /** Numero de la pantalla en design/specs y design/stitch. */
  id: string;
  title: string;
  route: string;
  module: ModuleId;
  layout: ScreenLayout;
  status: ScreenStatus;
  /** Ruta relativa a design/stitch (sin extension). */
  design: string;
  /** Aclaracion cuando la pantalla vive dentro de otra (p. ej. un modal). */
  note?: string;
};

export const modules: Record<ModuleId, { title: string; folder: string }> = {
  operacion: { title: "Operación: POS y Caja", folder: "01_Operacion_POS_Caja" },
  restaurante: { title: "Restaurante y Gastronomía", folder: "02_Restaurante_Gastronomia" },
  inventario: { title: "Inventario y Compras", folder: "03_Inventario_Compras" },
  ventas: { title: "Ventas y Finanzas", folder: "04_Ventas_Finanzas" },
  clientes: { title: "Clientes y Mayorista B2B", folder: "05_Clientes_Mayorista_B2B" },
  ecommerce: { title: "E-commerce Público", folder: "06_Ecommerce_Publico" },
  configuracion: { title: "Configuración y DIAN", folder: "07_Configuracion_DIAN" },
  marketing: { title: "Marketing y Fidelización", folder: "08_Marketing" },
};

function s(
  id: string,
  slug: string,
  title: string,
  route: string,
  module: ModuleId,
  layout: ScreenLayout,
  status: ScreenStatus = "diseno",
  note?: string,
): Screen {
  return {
    id,
    title,
    route,
    module,
    layout,
    status,
    design: `${modules[module].folder}/${id}-${slug}`,
    note,
  };
}

export const screens: Screen[] = [
  // Operacion
  s("13", "pos-mostrador", "POS Mostrador", "/pos", "operacion", "operacion", "react"),
  s("14", "cobro-y-comprobante", "Cobro y comprobante", "/pos", "operacion", "operacion", "react", "Modal dentro del POS (botón Cobrar)"),
  s("17", "caja", "Caja y arqueo", "/caja", "operacion", "operacion", "react"),
  // Restaurante
  s("21", "mapa-de-mesas", "Mapa de mesas", "/mesas", "restaurante", "operacion"),
  s("22", "comanda", "Comanda", "/comanda", "restaurante", "operacion"),
  s("23", "cocina-kds", "Cocina (KDS)", "/cocina", "restaurante", "operacion"),
  s("24", "cuenta-de-mesa", "Cuenta de mesa", "/mesas/cuenta", "restaurante", "operacion"),
  s("25", "recetas-e-insumos", "Recetas e insumos", "/recetas", "restaurante", "admin"),
  // Inventario y compras
  s("05", "stock-info", "Stock e inventario", "/inventario", "inventario", "admin", "react"),
  s("08", "compras", "Compras", "/compras", "inventario", "admin", "react"),
  s("15", "ficha-de-producto", "Ficha de producto", "/inventario/producto", "inventario", "admin", "react"),
  s("16", "movimientos-de-inventario", "Movimientos (kardex)", "/inventario/movimientos", "inventario", "admin", "react"),
  s("37", "modificadores", "Modificadores", "/inventario/modificadores", "inventario", "admin", "react"),
  s("38", "importar", "Importar productos", "/inventario/importar", "inventario", "admin", "react"),
  s("39", "etiquetas", "Etiquetas", "/inventario/etiquetas", "inventario", "admin", "react"),
  // Ventas y finanzas
  s("01", "panel-principal", "Panel principal", "/panel", "ventas", "admin", "react"),
  s("02", "resumen-de-ventas", "Resumen de ventas", "/ventas", "ventas", "admin"),
  s("03", "ventas-por-producto", "Ventas por producto", "/ventas/productos", "ventas", "admin"),
  s("04", "ordenes", "Órdenes", "/ordenes", "ventas", "admin", "react"),
  s("36", "devoluciones", "Devoluciones", "/ordenes/devoluciones", "ventas", "admin", "react"),
  s("06", "gastos", "Gastos", "/gastos", "ventas", "admin"),
  // Clientes y mayorista
  s("07", "clientes", "Clientes", "/clientes", "clientes", "admin"),
  s("26", "rutas", "Rutas", "/rutas", "clientes", "admin"),
  s("27", "pedido-mayorista", "Pedido mayorista", "/mayorista/pedido", "clientes", "operacion"),
  s("28", "app-del-repartidor", "App del repartidor", "/repartidor", "clientes", "operacion"),
  // E-commerce
  s("29", "catalogo-publico", "Catálogo público", "/tienda", "ecommerce", "publico"),
  s("30", "carrito-y-checkout", "Carrito y checkout", "/tienda/carrito", "ecommerce", "publico"),
  s("31", "seguimiento-del-pedido", "Seguimiento del pedido", "/tienda/seguimiento", "ecommerce", "publico"),
  // Configuracion
  s("09", "crear-cuenta", "Crear cuenta", "/registro", "configuracion", "publico"),
  s("11", "iniciar-sesion", "Iniciar sesión", "/login", "configuracion", "publico", "react"),
  s("12", "configuracion-inicial", "Configuración inicial", "/onboarding", "configuracion", "publico"),
  s("18", "negocio-y-sucursales", "Negocio y sucursales", "/configuracion/negocio", "configuracion", "admin", "react"),
  s("19", "usuarios-y-roles", "Usuarios y roles", "/configuracion/usuarios", "configuracion", "admin", "react"),
  s("20", "pagos-impuestos-y-facturacion", "Pagos, impuestos y DIAN", "/configuracion/facturacion", "configuracion", "admin"),
  s("32", "cajas", "Cajas registradoras", "/configuracion/cajas", "configuracion", "admin", "react"),
  s("33", "tarifas", "Tarifas y presentaciones", "/configuracion/tarifas", "configuracion", "admin", "react"),
  // Marketing
  s("34", "promociones", "Promociones", "/marketing/promociones", "marketing", "admin", "react"),
  s("35", "cupones", "Cupones", "/marketing/cupones", "marketing", "admin", "react"),
];

export function screenByRoute(route: string): Screen | undefined {
  return screens.find((screen) => screen.route === route);
}

/** Pantallas que aun se sirven como diseno de referencia. */
export const pendingScreens = screens.filter((screen) => screen.status === "diseno");
