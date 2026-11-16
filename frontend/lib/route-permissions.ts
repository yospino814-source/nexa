// Tabla de permisos requeridos por prefijo de ruta.
// Gana el prefijo mas largo y el usuario necesita al menos uno de sus permisos.

type RoutePermissionRule = {
  prefix: string;
  permissions: string[];
};

const ROUTE_PERMISSIONS: RoutePermissionRule[] = [
  { prefix: "/panel", permissions: ["reports:read"] },
  { prefix: "/ventas", permissions: ["reports:read"] },
  { prefix: "/pos", permissions: ["pos:sell"] },
  { prefix: "/pedidos", permissions: ["pos:sell"] },
  { prefix: "/conteo", permissions: ["inventory:adjust"] },
  { prefix: "/caja", permissions: ["cash:movement"] },
  { prefix: "/inventario", permissions: ["inventory:adjust"] },
  { prefix: "/inventario/vencimientos", permissions: ["inventory:read"] },
  { prefix: "/compras", permissions: ["purchases:create", "purchases:approve", "purchases:receive"] },
  { prefix: "/compras/proveedores", permissions: ["purchases:create"] },
  { prefix: "/compras/por-pagar", permissions: ["purchases:pay"] },
  { prefix: "/ordenes", permissions: ["cash:history"] },
  { prefix: "/ordenes/devoluciones", permissions: ["pos:refund"] },
  { prefix: "/gastos", permissions: ["expenses:create", "expenses:approve"] },
  { prefix: "/clientes", permissions: ["customers:read"] },
  { prefix: "/cartera", permissions: ["customers:read"] },
  { prefix: "/marketing", permissions: ["settings:write"] },
  { prefix: "/configuracion", permissions: ["settings:write"] },
  { prefix: "/configuracion/usuarios", permissions: ["users:manage"] },
  { prefix: "/configuracion/auditoria", permissions: ["reports:read"] },
];

export function canOpen(pathname: string, can: (code: string) => boolean): boolean {
  const clean = pathname.length > 1 && pathname.endsWith("/") ? pathname.slice(0, -1) : pathname;
  const match = ROUTE_PERMISSIONS.filter(
    (rule) => clean === rule.prefix || clean.startsWith(`${rule.prefix}/`),
  ).sort((a, b) => b.prefix.length - a.prefix.length)[0];

  if (!match) return true;
  return match.permissions.some((code) => can(code));
}
