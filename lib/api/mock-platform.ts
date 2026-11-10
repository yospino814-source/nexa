import type {
  AuditEntry,
  AuditFilter,
  BranchSettings,
  Invitation,
  InviteUserInput,
  LoginAttemptInput,
  Permission,
  Role,
  RolePermissionItem,
  RolePermissionsResponse,
  TenantUser,
  UpdateRolePermissionsInput,
  Uuid,
} from "./types";
import { ApiError } from "./client";
import { MOCK_BRANCH_ID } from "./mock";

const BRANCH_CENTRO = MOCK_BRANCH_ID;
const BRANCH_ASADERO = "a0eebc99-9c0b-4ef8-bb6d-6bb9bd380a12";

export const mockBranchesList = [
  { id: BRANCH_CENTRO, name: "Tienda Centro", code: "TC", city: "Medellín", activeTypes: ["Tienda", "Tienda virtual"], userCount: 4 },
  { id: BRANCH_ASADERO, name: "Asadero El Vecino", code: "AV", city: "Medellín", activeTypes: ["Restaurante"], userCount: 4 },
];

export const mockBusinessInfo = {
  commercial_name: "Supermercado El Vecino",
  legal_name: "Supermercado El Vecino S.A.S",
  tax_id: "901.234.567-8",
  phone: "+57 (604) 444-5566",
  email: "administracion@elvecino.co",
  address: "Cra. 43A #1-50, Medellín, Antioquia",
};

const mockUsersData: TenantUser[] = [
  {
    id: "u1111111-1111-4111-8111-111111111111",
    email: "carlos@elvecino.co",
    full_name: "Carlos Restrepo",
    role: "DUENO",
    branches: [
      { id: BRANCH_CENTRO, name: "Tienda Centro" },
      { id: BRANCH_ASADERO, name: "Asadero El Vecino" },
    ],
    active: true,
    created_at: "2026-01-10T08:00:00Z",
  },
  {
    id: "u2222222-2222-4222-8222-222222222222",
    email: "camila@elvecino.co",
    full_name: "Maria Camila Gómez",
    role: "ADMIN_CAJA",
    branches: [{ id: BRANCH_CENTRO, name: "Tienda Centro" }],
    active: true,
    created_at: "2026-01-15T09:30:00Z",
  },
  {
    id: "u3333333-3333-4333-8333-333333333333",
    email: "juanp@elvecino.co",
    full_name: "Juan Pablo Montoya",
    role: "CAJERO",
    branches: [{ id: BRANCH_CENTRO, name: "Tienda Centro" }],
    active: true,
    created_at: "2026-02-01T10:00:00Z",
  },
  {
    id: "u4444444-4444-4444-8444-444444444444",
    email: "diana@elvecino.co",
    full_name: "Diana Marcela Uribe",
    role: "CAJERO",
    branches: [{ id: BRANCH_ASADERO, name: "Asadero El Vecino" }],
    active: true,
    created_at: "2026-02-05T11:15:00Z",
  },
  {
    id: "u5555555-5555-4555-8555-555555555555",
    email: "andres@elvecino.co",
    full_name: "Andrés Felipe Ospina",
    role: "MESERO",
    branches: [{ id: BRANCH_ASADERO, name: "Asadero El Vecino" }],
    active: true,
    created_at: "2026-02-10T14:00:00Z",
  },
  {
    id: "u6666666-6666-4666-8666-666666666666",
    email: "luz@elvecino.co",
    full_name: "Luz Marina Henao",
    role: "COCINA",
    branches: [{ id: BRANCH_ASADERO, name: "Asadero El Vecino" }],
    active: true,
    created_at: "2026-02-12T07:45:00Z",
  },
  {
    id: "u7777777-7777-4777-8777-777777777777",
    email: "jhon@elvecino.co",
    full_name: "Jhon Jairo Moreno",
    role: "BODEGA",
    branches: [
      { id: BRANCH_CENTRO, name: "Tienda Centro" },
      { id: BRANCH_ASADERO, name: "Asadero El Vecino" },
    ],
    active: true,
    created_at: "2026-02-15T08:30:00Z",
  },
  {
    id: "u8888888-8888-4888-8888-888888888888",
    email: "mateo@elvecino.co",
    full_name: "Mateo Álvarez",
    role: "REPARTIDOR",
    branches: [{ id: BRANCH_CENTRO, name: "Tienda Centro" }],
    active: true,
    created_at: "2026-02-20T09:00:00Z",
  },
];

const mockInvitationsData: Invitation[] = [
  {
    id: "i1111111-1111-4111-8111-111111111111",
    email: "stella.duque@gmail.com",
    full_name: "Stella Duque",
    role: "CAJERO",
    branch_id: BRANCH_CENTRO,
    branch_name: "Sede Centro",
    status: "PENDIENTE",
    created_at: "2026-03-01T16:20:00Z",
  },
];

export const PERMISSION_CATALOG: Permission[] = [
  { code: "pos:sell", module: "pos", description: "Vender en mostrador" },
  { code: "pos:order", module: "pos", description: "Tomar pedidos y comandas de mesa" },
  { code: "pos:kitchen", module: "pos", description: "Ver y preparar pedidos en pantalla de cocina" },
  { code: "pos:price_override", module: "pos", description: "Modificar precio en venta" },
  { code: "pos:discount", module: "pos", description: "Aplicar descuento en venta" },
  { code: "pos:custom_item", module: "pos", description: "Agregar producto libre o no catalogado" },
  { code: "pos:refund", module: "pos", description: "Autorizar devoluciones o notas crédito" },
  { code: "cash:open", module: "cash", description: "Abrir turno de caja" },
  { code: "cash:close", module: "cash", description: "Cerrar turno de caja y hacer arqueo" },
  { code: "cash:movement", module: "cash", description: "Registrar ingresos o retiros de efectivo" },
  { code: "cash:history", module: "cash", description: "Ver historial y auditoría de turnos de caja" },
  { code: "inventory:read", module: "inventory", description: "Consultar existencias y kardex" },
  { code: "inventory:adjust", module: "inventory", description: "Realizar ajustes y conteos de stock" },
  { code: "inventory:adjust_approve", module: "inventory", description: "Aprobar ajustes de stock elevados" },
  { code: "inventory:transfer", module: "inventory", description: "Trasladar mercancía entre sucursales" },
  { code: "purchases:create", module: "purchases", description: "Crear órdenes de compra a proveedores" },
  { code: "purchases:approve", module: "purchases", description: "Aprobar facturas de compra" },
  { code: "purchases:receive", module: "purchases", description: "Recibir mercancía en almacén e ingresar al kardex" },
  { code: "purchases:pay", module: "purchases", description: "Registrar pagos y egresos a proveedores" },
  { code: "customers:read", module: "customers", description: "Consultar directorio de clientes y saldos" },
  { code: "customers:write", module: "customers", description: "Crear y editar fichas de clientes" },
  { code: "credit:sell", module: "credit", description: "Vender a crédito (fiado) según cupo" },
  { code: "credit:collect", module: "credit", description: "Cobrar abonos de cartera y cuotas" },
  { code: "credit:write_off", module: "credit", description: "Castigar o condonar cartera vencida" },
  { code: "reports:read", module: "reports", description: "Ver reportes gerenciales y de ventas" },
  { code: "settings:write", module: "settings", description: "Modificar ajustes del negocio y sucursales" },
  { code: "users:manage", module: "users", description: "Invitar usuarios y administrar roles" },
];

const CAJERO_BASE = [
  "inventory:read",
  "pos:order",
  "pos:sell",
  "cash:movement",
  "cash:open",
  "cash:close",
  "cash:history",
  "pos:discount",
  "pos:custom_item",
  "customers:read",
  "customers:write",
  "credit:collect",
];

const BASE_ROLE_PERMISSIONS: Record<Role, string[]> = {
  DUENO: PERMISSION_CATALOG.map((p) => p.code),
  ADMIN_CAJA: PERMISSION_CATALOG.map((p) => p.code).filter((c) => c !== "settings:write" && c !== "users:manage"),
  CAJERO: [...CAJERO_BASE],
  MESERO: ["inventory:read", "pos:order", "customers:read"],
  COCINA: ["inventory:read", "pos:kitchen"],
  SUPERVISOR: [
    ...CAJERO_BASE,
    "pos:refund",
    "inventory:adjust",
    "reports:read",
  ],
  BODEGA: [
    "inventory:read",
    "inventory:adjust",
    "inventory:adjust_approve",
    "inventory:transfer",
    "purchases:receive",
  ],
  REPARTIDOR: ["pos:sell", "inventory:read"],
};

// Almacén de overrides en memoria para el modo mock
const mockRoleOverrides = new Map<Role, Map<string, boolean>>();

const mockBranchSettings: Record<string, BranchSettings> = {
  [BRANCH_CENTRO]: {
    pos: {
      allow_price_override: false,
      max_discount_pct: 15,
      refund_requires_approval: true,
      allow_anonymous_sales: true,
      auto_print: true,
    },
    cash: {
      require_session: true,
      close_tolerance: "0.00",
      blind_close: true,
    },
    inventory: {
      allow_negative_stock: false,
      track_raw_materials: true,
      low_stock_default: 5,
      costing: "PROMEDIO",
    },
    tips: {
      enabled: false,
      suggested_pct: [10],
      distribute_evenly: true,
    },
    print: {
      ticket_width_mm: 80,
      header_text: "Supermercado El Vecino S.A.S\nNIT: 901.234.567-8\nIVA Régimen Común",
      footer_text: "¡Gracias por su compra!\nConserve esta tirilla para cualquier reclamo o garantía.",
    },
  },
};

const mockAuditEntries: AuditEntry[] = [
  {
    id: "a101",
    tenant_id: "t1",
    branch_id: BRANCH_CENTRO,
    user_id: "u3333333-3333-4333-8333-333333333333",
    action: "CIERRE_CAJA",
    entity: "cash_session",
    entity_id: "s-2026-10-31-01",
    details: { counted_cash: "742300.00", expected_cash: "742300.00", diff: "0.00", status: "CUADRADO" },
    ip_address: "192.168.1.102",
    created_at: "2026-10-31T20:30:00Z",
  },
  {
    id: "a102",
    tenant_id: "t1",
    branch_id: BRANCH_CENTRO,
    user_id: "u1111111-1111-4111-8111-111111111111",
    action: "CAMBIO_AJUSTES",
    entity: "branch_settings",
    entity_id: BRANCH_CENTRO,
    details: { field: "pos.max_discount_pct", before: 10, after: 15 },
    ip_address: "192.168.1.100",
    created_at: "2026-10-31T18:45:00Z",
  },
  {
    id: "a103",
    tenant_id: "t1",
    branch_id: BRANCH_CENTRO,
    user_id: "u7777777-7777-4777-8777-777777777777",
    action: "AJUSTE_INVENTARIO",
    entity: "inventory_movement",
    entity_id: "m-8821",
    details: { product: "Arroz Diana 1kg", delta: "-2.00", reason: "Empaque roto en estantería" },
    ip_address: "192.168.1.108",
    created_at: "2026-10-31T15:20:00Z",
  },
  {
    id: "a104",
    tenant_id: "t1",
    branch_id: BRANCH_CENTRO,
    user_id: "u1111111-1111-4111-8111-111111111111",
    action: "INVITACION_USUARIO",
    entity: "invitation",
    entity_id: "i1111111-1111-4111-8111-111111111111",
    details: { email: "stella.duque@gmail.com", role: "CAJERO" },
    ip_address: "192.168.1.100",
    created_at: "2026-10-31T11:00:00Z",
  },
  {
    id: "a105",
    tenant_id: "t1",
    branch_id: BRANCH_CENTRO,
    user_id: "u3333333-3333-4333-8333-333333333333",
    action: "APERTURA_CAJA",
    entity: "cash_session",
    entity_id: "s-2026-10-31-01",
    details: { opening_amount: "100000.00" },
    ip_address: "192.168.1.102",
    created_at: "2026-10-31T08:00:00Z",
  },
];

export function mockUsersList(): { users: TenantUser[]; invitations: Invitation[] } {
  return {
    users: structuredClone(mockUsersData),
    invitations: structuredClone(mockInvitationsData),
  };
}

export function mockInviteUser(input: InviteUserInput): Invitation {
  const inv: Invitation = {
    id: "inv-" + Date.now(),
    email: input.email,
    full_name: input.full_name,
    role: input.role,
    branch_id: input.branch_id ?? null,
    branch_name: input.branch_id ? "Sede Centro" : null,
    status: "PENDIENTE",
    created_at: new Date().toISOString(),
  };
  mockInvitationsData.push(inv);
  mockAuditEntries.unshift({
    id: "a-" + Date.now(),
    tenant_id: "t1",
    branch_id: input.branch_id ?? null,
    user_id: "u1111111-1111-4111-8111-111111111111",
    action: "USUARIO_INVITADO",
    entity: "invitation",
    entity_id: inv.id,
    details: { email: input.email, role: input.role },
    ip_address: "127.0.0.1",
    created_at: new Date().toISOString(),
  });
  return inv;
}

export function mockUpdateUserRole(userId: Uuid, role: Role): TenantUser {
  const user = mockUsersData.find((u) => u.id === userId);
  if (!user) throw new Error("Usuario no encontrado");
  const oldRole = user.role;
  user.role = role;
  mockAuditEntries.unshift({
    id: "a-" + Date.now(),
    tenant_id: "t1",
    branch_id: user.branches[0]?.id ?? null,
    user_id: "u1111111-1111-4111-8111-111111111111",
    action: "CAMBIO_ROL",
    entity: "users",
    entity_id: userId,
    details: { user: user.full_name, old_role: oldRole, new_role: role },
    ip_address: "127.0.0.1",
    created_at: new Date().toISOString(),
  });
  return structuredClone(user);
}

export function mockUpdateUserStatus(userId: Uuid, active: boolean): TenantUser {
  const user = mockUsersData.find((u) => u.id === userId);
  if (!user) throw new Error("Usuario no encontrado");
  user.active = active;
  mockAuditEntries.unshift({
    id: "a-" + Date.now(),
    tenant_id: "t1",
    branch_id: user.branches[0]?.id ?? null,
    user_id: "u1111111-1111-4111-8111-111111111111",
    action: active ? "ACTIVAR_USUARIO" : "DESACTIVAR_USUARIO",
    entity: "users",
    entity_id: userId,
    details: { user: user.full_name, active },
    ip_address: "127.0.0.1",
    created_at: new Date().toISOString(),
  });
  return structuredClone(user);
}

export function mockPermissionsList(): { permissions: Permission[]; roles: Role[] } {
  const roles: Role[] = ["DUENO", "ADMIN_CAJA", "CAJERO", "MESERO", "COCINA", "SUPERVISOR", "BODEGA", "REPARTIDOR"];
  return {
    permissions: structuredClone(PERMISSION_CATALOG),
    roles,
  };
}

export function mockRolePermissions(role: Role): RolePermissionsResponse {
  const base = new Set(BASE_ROLE_PERMISSIONS[role] ?? []);
  const overrides = mockRoleOverrides.get(role);

  const items: RolePermissionItem[] = PERMISSION_CATALOG.map((perm) => {
    let allowed = base.has(perm.code);
    let isOverride = false;
    if (overrides && overrides.has(perm.code)) {
      allowed = overrides.get(perm.code)!;
      isOverride = true;
    }
    return {
      code: perm.code,
      module: perm.module,
      description: perm.description,
      allowed,
      is_override: isOverride,
    };
  });

  return {
    role,
    permissions: items,
  };
}

export function mockUpdateRolePermissions(role: Role, input: UpdateRolePermissionsInput): RolePermissionsResponse {
  if (role === "DUENO") {
    throw new Error("No se pueden modificar los permisos del Dueño");
  }
  let roleMap = mockRoleOverrides.get(role);
  if (!roleMap) {
    roleMap = new Map();
    mockRoleOverrides.set(role, roleMap);
  }
  for (const item of input.overrides) {
    roleMap.set(item.permission_code, item.allowed);
  }
  mockAuditEntries.unshift({
    id: "a-" + Date.now(),
    tenant_id: "t1",
    branch_id: null,
    user_id: "u1111111-1111-4111-8111-111111111111",
    action: "CAMBIO_PERMISOS_ROL",
    entity: "roles",
    entity_id: role,
    details: { role, overrides_count: input.overrides.length },
    ip_address: "127.0.0.1",
    created_at: new Date().toISOString(),
  });
  return mockRolePermissions(role);
}

export function mockGetSettings(branchId: Uuid): BranchSettings {
  const current = mockBranchSettings[branchId] ?? mockBranchSettings[BRANCH_CENTRO];
  return structuredClone(current);
}

export function mockUpdateSettings(branchId: Uuid, settings: BranchSettings): BranchSettings {
  mockBranchSettings[branchId] = structuredClone(settings);
  mockAuditEntries.unshift({
    id: "a-" + Date.now(),
    tenant_id: "t1",
    branch_id: branchId,
    user_id: "u1111111-1111-4111-8111-111111111111",
    action: "CAMBIO_AJUSTES",
    entity: "branch_settings",
    entity_id: branchId,
    details: { updated_at: new Date().toISOString() },
    ip_address: "127.0.0.1",
    created_at: new Date().toISOString(),
  });
  return structuredClone(settings);
}

export function mockGetAudit(filter?: AuditFilter): AuditEntry[] {
  let list = structuredClone(mockAuditEntries);
  if (filter?.action) {
    list = list.filter((e) => e.action === filter.action);
  }
  if (filter?.user_id) {
    list = list.filter((e) => e.user_id === filter.user_id);
  }
  if (filter?.branch_id) {
    list = list.filter((e) => e.branch_id === filter.branch_id);
  }
  if (filter?.limit) {
    list = list.slice(0, filter.limit);
  }
  return list;
}

interface MockLoginAttempt {
  email: string;
  success: boolean;
  reason?: string;
  at: number;
}

const mockLoginAttempts: MockLoginAttempt[] = [];

export function mockRecordLoginAttempt(input: LoginAttemptInput): { ok: boolean; failed_attempts_in_window: number } {
  const email = input.email.trim().toLowerCase();
  if (!email) {
    throw new ApiError(422, "email is required");
  }
  const now = Date.now();
  const windowStart = now - 15 * 60 * 1000;

  if (typeof input.success === "boolean") {
    mockLoginAttempts.push({
      email,
      success: input.success,
      reason: input.failure_reason,
      at: now,
    });
  }

  const recentFailures = mockLoginAttempts.filter(
    (a) => a.email === email && !a.success && a.at >= windowStart
  ).length;

  if (recentFailures >= 5) {
    throw new ApiError(429, "too many requests: demasiados intentos fallidos en los últimos 15 minutos");
  }

  return { ok: true, failed_attempts_in_window: recentFailures };
}



/**
 * Meseros para el selector de propina (GET /v1/waiters). En el modo demo se
 * listan todos los meseros activos, sin filtrar por sucursal, para que el
 * selector tenga datos en cualquier sucursal de prueba.
 */
export function mockWaiters(): { user_id: string; full_name: string }[] {
  return mockUsersData
    .filter((u) => u.role === "MESERO" && u.active)
    .map((u) => ({ user_id: u.id, full_name: u.full_name ?? u.email ?? "Mesero" }));
}
