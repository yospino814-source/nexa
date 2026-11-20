"use client";

import { useMemo, useState } from "react";
import {
  Check,
  Copy,
  Lock,
  Mail,
  Plus,
  RefreshCw,
  Search,
  Shield,
  ShieldCheck,
  UserCheck,
  UserX,
  Users as UsersIcon,
  X,
} from "lucide-react";

import {
  api,
  friendlyError,
  type Role,
  type RolePermissionItem,
  type TenantUser,
} from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { ASSIGNABLE_ROLES, ROLE_LABELS, initials, useSession } from "@/lib/stores/session";
import { cn } from "@/lib/utils";

const ROLE_DESCRIPTIONS: Record<Role, string> = {
  DUENO: "Ve todo y configura el negocio",
  ADMIN_CAJA: "Abre y cierra caja, ve reportes de su sucursal",
  CAJERO: "Vende y cobra en el mostrador",
  MESERO: "Abre mesas y toma comandas",
  COCINA: "Ve y despacha los platos en la pantalla de cocina",
  REPARTIDOR: "Ve sus entregas del día y cobra contra entrega",
  BODEGA: "Recibe compras y ajusta inventario",
  SUPERVISOR: "Supervisa turnos, autoriza excepciones y revisa arqueos",
};

const MODULE_LABELS: Record<string, string> = {
  pos: "Punto de Venta (POS)",
  cash: "Caja y Efectivo",
  inventory: "Inventario y Stock",
  purchases: "Compras y Proveedores",
  customers: "Clientes y Fichas",
  credit: "Crédito y Cartera",
  reports: "Reportes Gerenciales",
  settings: "Configuración del Negocio",
  users: "Gestión de Usuarios",
};

export function UsersScreen() {
  const branches = useSession((s) => s.branches);
  const currentEmail = useSession((s) => s.email);
  const isOwner = branches.some((b) => b.role === "DUENO");
  const availableRoles: Role[] = isOwner ? [...ASSIGNABLE_ROLES, "DUENO"] : ASSIGNABLE_ROLES;

  const { data: businessInfo } = useApi(() => api.business.get(), "business-info");
  const businessName = businessInfo?.business_name || businessInfo?.name || branches[0]?.name || "Nexa";

  const [activeTab, setActiveTab] = useState<"usuarios" | "roles">("usuarios");
  const [query, setQuery] = useState("");
  const [roleFilter, setRoleFilter] = useState<Role | "">("");
  const [branchFilter, setBranchFilter] = useState<string>("");

  const [inviteModalOpen, setInviteModalOpen] = useState(false);
  const [changeRoleUser, setChangeRoleUser] = useState<TenantUser | null>(null);
  const [selectedRoleForPerms, setSelectedRoleForPerms] = useState<Role>("CAJERO");
  const [overrides, setOverrides] = useState<Record<string, boolean>>({});
  const [savingPerms, setSavingPerms] = useState(false);
  const [toast, setToast] = useState<string | null>(null);

  // Invite form state
  const [inviteName, setInviteName] = useState("");
  const [inviteEmail, setInviteEmail] = useState("");
  const [inviteRole, setInviteRole] = useState<Role>("CAJERO");
  const [inviteBranch, setInviteBranch] = useState<string>("ALL");
  const [submittingInvite, setSubmittingInvite] = useState(false);
  const [inviteError, setInviteError] = useState<string | null>(null);
  const [createdInvite, setCreatedInvite] = useState<{
    fullName: string;
    email: string;
    role: Role;
    message: string;
  } | null>(null);
  const [copiedMessage, setCopiedMessage] = useState(false);

  function openInviteModal() {
    setInviteName("");
    setInviteEmail("");
    setInviteRole("CAJERO");
    setInviteBranch("ALL");
    setInviteError(null);
    setCreatedInvite(null);
    setCopiedMessage(false);
    setInviteModalOpen(true);
  }

  const { data, reload } = useApi(() => api.users.list(), "users-list");
  const { data: rolePermsData, reload: reloadRolePerms } = useApi(
    () => api.permissions.getRole(selectedRoleForPerms),
    `role-perms-${selectedRoleForPerms}`,
  );

  const users = useMemo(() => data?.users ?? [], [data]);
  const invitations = useMemo(() => data?.invitations ?? [], [data]);

  const filteredUsers = useMemo(() => {
    const q = query.trim().toLowerCase();
    return users.filter((u) => {
      const matchesQ = !q || (u.full_name?.toLowerCase().includes(q) ?? false) || (u.email?.toLowerCase().includes(q) ?? false);
      const matchesRole = !roleFilter || u.role === roleFilter;
      const matchesBranch = !branchFilter || u.branches.some((b) => b.id === branchFilter || branchFilter === "ALL");
      return matchesQ && matchesRole && matchesBranch;
    });
  }, [users, query, roleFilter, branchFilter]);

  const filteredInvitations = useMemo(() => {
    const q = query.trim().toLowerCase();
    return invitations.filter((inv) => {
      const matchesQ = !q || inv.email.toLowerCase().includes(q);
      const matchesRole = !roleFilter || inv.role === roleFilter;
      return matchesQ && matchesRole;
    });
  }, [invitations, query, roleFilter]);

  function notify(msg: string) {
    setToast(msg);
    setTimeout(() => setToast(null), 4000);
  }

  async function handleSendInvite(e: React.FormEvent) {
    e.preventDefault();
    const nameTrimmed = inviteName.trim();
    if (nameTrimmed.length < 2 || nameTrimmed.length > 80) {
      setInviteError("El nombre debe tener entre 2 y 80 caracteres.");
      return;
    }
    const emailTrimmed = inviteEmail.trim();
    if (!emailTrimmed || !emailTrimmed.includes("@")) {
      setInviteError("Ingresá un correo electrónico válido");
      return;
    }
    setSubmittingInvite(true);
    setInviteError(null);
    try {
      const branchId = inviteBranch === "ALL" ? null : inviteBranch;
      await api.users.invite({
        email: emailTrimmed,
        full_name: nameTrimmed,
        role: inviteRole,
        branch_id: branchId,
      });

      const origin = typeof window !== "undefined" ? window.location.origin : "";
      const roleLabel = ROLE_LABELS[inviteRole] ?? inviteRole;
      const message = `Hola ${nameTrimmed}, te invité a ${businessName} en Nexa POS como ${roleLabel}. Para entrar, creá tu cuenta con este correo: ${emailTrimmed}, en ${origin}/registro?correo=${encodeURIComponent(emailTrimmed)}. Si ya tenés cuenta, solo iniciá sesión.`;
      setCreatedInvite({
        fullName: nameTrimmed,
        email: emailTrimmed,
        role: inviteRole,
        message,
      });
      reload();
    } catch (err: unknown) {
      setInviteError(friendlyError(err));
    } finally {
      setSubmittingInvite(false);
    }
  }

  async function handleToggleStatus(user: TenantUser) {
    if (user.email === currentEmail && user.role === "DUENO") {
      notify("No podés desactivar tu propia cuenta de dueño");
      return;
    }
    try {
      await api.users.updateStatus(user.id, { active: !user.active });
      notify(user.active ? "Usuario desactivado" : "Usuario activado");
      reload();
    } catch {
      notify("No se pudo cambiar el estado del usuario");
    }
  }

  async function handleChangeRole(user: TenantUser, newRole: Role) {
    if (user.email === currentEmail && user.role === "DUENO" && newRole !== "DUENO") {
      notify("No podés quitarte tu propio rol de dueño");
      return;
    }
    try {
      await api.users.updateRole(user.id, { role: newRole });
      notify(`Rol actualizado a ${ROLE_LABELS[newRole]}`);
      setChangeRoleUser(null);
      reload();
    } catch {
      notify("No se pudo actualizar el rol");
    }
  }

  async function handleSaveRolePermissions() {
    if (selectedRoleForPerms === "DUENO") {
      notify("Los permisos del Dueño no se pueden modificar");
      return;
    }
    setSavingPerms(true);
    try {
      const overridesPayload = Object.entries(overrides).map(([permission_code, allowed]) => ({
        permission_code,
        allowed,
      }));
      await api.permissions.updateRole(selectedRoleForPerms, { overrides: overridesPayload });
      setOverrides({});
      notify(`Permisos de ${ROLE_LABELS[selectedRoleForPerms]} actualizados`);
      reloadRolePerms();
    } catch {
      notify("Error al actualizar permisos");
    } finally {
      setSavingPerms(false);
    }
  }

  // Group permissions by module for the matrix
  const permissionsByModule = useMemo(() => {
    const map = new Map<string, RolePermissionItem[]>();
    if (!rolePermsData?.permissions) return map;
    for (const p of rolePermsData.permissions) {
      const list = map.get(p.module) ?? [];
      list.push(p);
      map.set(p.module, list);
    }
    return map;
  }, [rolePermsData]);

  const userCountsByRole = useMemo(() => {
    const counts: Record<Role, number> = {
      DUENO: 0,
      ADMIN_CAJA: 0,
      CAJERO: 0,
      MESERO: 0,
      COCINA: 0,
      SUPERVISOR: 0,
      BODEGA: 0,
      REPARTIDOR: 0,
    };
    for (const u of users) {
      if (counts[u.role] !== undefined) counts[u.role]++;
    }
    return counts;
  }, [users]);

  return (
    <div className="space-y-6 p-6 lg:p-10">
      {/* Toast notification */}
      {toast && (
        <div className="fixed top-6 right-6 z-50 rounded-2xl bg-slate-900 px-5 py-3 text-sm font-medium text-white shadow-xl transition-all">
          {toast}
        </div>
      )}

      {/* Header */}
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Usuarios y roles</h1>
          <p className="mt-1 text-slate-500">
            Administrá el equipo, las sucursales asignadas y los permisos de acceso al sistema.
          </p>
        </div>

        {/* Tabs toggle */}
        <div className="flex rounded-full border border-slate-200 bg-white p-1 shadow-sm">
          <button
            onClick={() => setActiveTab("usuarios")}
            className={cn(
              "flex items-center gap-2 rounded-full px-5 py-2 text-sm font-medium transition-colors",
              activeTab === "usuarios"
                ? "bg-slate-900 text-white shadow-sm"
                : "text-slate-600 hover:text-slate-900",
            )}
          >
            <UsersIcon className="size-4" />
            Usuarios ({users.length + invitations.length})
          </button>
          <button
            onClick={() => setActiveTab("roles")}
            className={cn(
              "flex items-center gap-2 rounded-full px-5 py-2 text-sm font-medium transition-colors",
              activeTab === "roles"
                ? "bg-slate-900 text-white shadow-sm"
                : "text-slate-600 hover:text-slate-900",
            )}
          >
            <ShieldCheck className="size-4" />
            Roles y Permisos
          </button>
        </div>
      </div>

      {activeTab === "usuarios" ? (
        <>
          {/* Action Bar */}
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="flex flex-wrap items-center gap-3">
              <div className="relative w-64 md:w-80">
                <Search className="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-slate-400" />
                <input
                  type="text"
                  placeholder="Buscar por nombre o correo..."
                  value={query}
                  onChange={(e) => setQuery(e.target.value)}
                  className="h-10 w-full rounded-full border border-slate-200 bg-white pr-4 pl-9 text-sm outline-none transition focus:border-slate-900 focus:ring-1 focus:ring-slate-900"
                />
              </div>

              {/* Role Filter */}
              <select
                value={roleFilter}
                onChange={(e) => setRoleFilter(e.target.value as Role | "")}
                className="h-10 rounded-full border border-slate-200 bg-white px-4 text-sm font-medium text-slate-700 outline-none hover:bg-slate-50 focus:border-slate-900"
              >
                <option value="">Todos los roles</option>
                {(["DUENO", ...ASSIGNABLE_ROLES] as const).map((code) => (
                  <option key={code} value={code}>
                    {ROLE_LABELS[code]}
                  </option>
                ))}
              </select>

              {/* Branch Filter */}
              <select
                value={branchFilter}
                onChange={(e) => setBranchFilter(e.target.value)}
                className="h-10 rounded-full border border-slate-200 bg-white px-4 text-sm font-medium text-slate-700 outline-none hover:bg-slate-50 focus:border-slate-900"
              >
                <option value="">Todas las sucursales</option>
                {branches.map((b) => (
                  <option key={b.id} value={b.id}>
                    {b.name}
                  </option>
                ))}
              </select>
            </div>

            <button
              onClick={openInviteModal}
              className="flex h-10 items-center gap-2 rounded-full bg-slate-900 px-5 text-sm font-semibold text-white shadow-sm transition hover:bg-slate-800"
            >
              <Plus className="size-4" />
              Invitar usuario
            </button>
          </div>

          {/* Table */}
          <div className="overflow-hidden rounded-3xl border border-slate-200 bg-white shadow-sm">
            <table className="w-full text-left text-sm">
              <thead className="border-b border-slate-100 bg-slate-50 text-xs font-semibold text-slate-500 uppercase">
                <tr>
                  <th className="py-3.5 pr-4 pl-6">Usuario</th>
                  <th className="px-4 py-3.5">Rol</th>
                  <th className="px-4 py-3.5">Sucursal</th>
                  <th className="px-4 py-3.5">Estado</th>
                  <th className="py-3.5 pr-6 pl-4 text-right">Acciones</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100">
                {filteredUsers.map((user) => (
                  <tr key={user.id} className="transition hover:bg-slate-50/50">
                    <td className="py-4 pr-4 pl-6">
                      <div className="flex items-center gap-3">
                        <div className="flex size-10 shrink-0 items-center justify-center rounded-full bg-slate-100 font-bold text-slate-700">
                          {initials(user.full_name)}
                        </div>
                        <div>
                          <div className="font-semibold text-slate-900">{user.full_name ?? "Sin nombre"}</div>
                          <div className="text-xs text-slate-500">{user.email ?? "Sin correo"}</div>
                        </div>
                      </div>
                    </td>
                    <td className="px-4 py-4">
                      <span className="inline-flex items-center rounded-full bg-slate-100 px-3 py-1 text-xs font-semibold text-slate-800">
                        {ROLE_LABELS[user.role] ?? user.role}
                      </span>
                    </td>
                    <td className="px-4 py-4 text-slate-600">
                      {user.branches.length === branches.length
                        ? "Todas las sucursales"
                        : user.branches.map((b) => b.name).join(", ") || "Sin sucursal"}
                    </td>
                    <td className="px-4 py-4">
                      {user.active ? (
                        <span className="inline-flex items-center gap-1.5 rounded-full border border-emerald-200 bg-emerald-50 px-2.5 py-0.5 text-xs font-medium text-emerald-700">
                          <span className="size-1.5 rounded-full bg-emerald-500" />
                          Activo
                        </span>
                      ) : (
                        <span className="inline-flex items-center gap-1.5 rounded-full border border-slate-200 bg-slate-100 px-2.5 py-0.5 text-xs font-medium text-slate-600">
                          <span className="size-1.5 rounded-full bg-slate-400" />
                          Inactivo
                        </span>
                      )}
                    </td>
                    <td className="py-4 pr-6 pl-4 text-right">
                      <div className="flex items-center justify-end gap-2">
                        <button
                          onClick={() => setChangeRoleUser(user)}
                          className="rounded-full border border-slate-200 bg-white px-3 py-1 text-xs font-medium text-slate-700 hover:bg-slate-50"
                        >
                          Cambiar rol
                        </button>
                        <button
                          onClick={() => handleToggleStatus(user)}
                          title={user.active ? "Desactivar acceso" : "Activar acceso"}
                          className={cn(
                            "rounded-full p-1.5 transition",
                            user.active
                              ? "text-slate-400 hover:bg-rose-50 hover:text-rose-600"
                              : "text-slate-400 hover:bg-emerald-50 hover:text-emerald-600",
                          )}
                        >
                          {user.active ? <UserX className="size-4" /> : <UserCheck className="size-4" />}
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}

                {/* Pending invitations */}
                {filteredInvitations.map((inv) => (
                  <tr key={inv.id} className="bg-amber-50/20 transition hover:bg-amber-50/40">
                    <td className="py-4 pr-4 pl-6">
                      <div className="flex items-center gap-3">
                        <div className="flex size-10 shrink-0 items-center justify-center rounded-full bg-amber-100 font-bold text-amber-700">
                          <Mail className="size-4" />
                        </div>
                        <div>
                          <div className="font-semibold text-slate-900">{inv.email}</div>
                          <div className="text-xs text-amber-600">Invitación enviada</div>
                        </div>
                      </div>
                    </td>
                    <td className="px-4 py-4">
                      <span className="inline-flex items-center rounded-full bg-slate-100 px-3 py-1 text-xs font-semibold text-slate-800">
                        {ROLE_LABELS[inv.role] ?? inv.role}
                      </span>
                    </td>
                    <td className="px-4 py-4 text-slate-600">
                      {inv.branch_name ?? "Todas las sucursales"}
                    </td>
                    <td className="px-4 py-4">
                      <span className="inline-flex items-center gap-1.5 rounded-full border border-amber-200 bg-amber-50 px-2.5 py-0.5 text-xs font-medium text-amber-700">
                        <span className="size-1.5 rounded-full bg-amber-500 animate-pulse" />
                        Pendiente
                      </span>
                    </td>
                    <td className="py-4 pr-6 pl-4 text-right">
                      <div className="flex items-center justify-end gap-2">
                        <button
                          type="button"
                          onClick={() => {
                            const origin = typeof window !== "undefined" ? window.location.origin : "";
                            const roleLabel = ROLE_LABELS[inv.role] ?? inv.role;
                            const msg = `Hola${inv.full_name ? " " + inv.full_name : ""}, te invité a ${businessName} en Nexa POS como ${roleLabel}. Para entrar, creá tu cuenta con este correo: ${inv.email}, en ${origin}/registro?correo=${encodeURIComponent(inv.email)}. Si ya tenés cuenta, solo iniciá sesión.`;
                            navigator.clipboard.writeText(msg);
                            notify("Mensaje copiado");
                          }}
                          className="rounded-full border border-slate-200 bg-white px-3 py-1 text-xs font-medium text-slate-700 hover:bg-slate-50"
                        >
                          Copiar mensaje
                        </button>
                        <button
                          type="button"
                          onClick={async () => {
                            if (confirm(`¿Revocar la invitación de ${inv.email}?`)) {
                              try {
                                await api.users.revokeInvitation(inv.id);
                                reload();
                                notify("Invitación revocada");
                              } catch (err: unknown) {
                                notify(friendlyError(err));
                              }
                            }
                          }}
                          className="rounded-full border border-rose-200 bg-rose-50 px-3 py-1 text-xs font-medium text-rose-700 hover:bg-rose-100"
                        >
                          Revocar
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </>
      ) : (
        /* Tab Roles y Permisos */
        <div className="space-y-8">
          {/* Grid of Roles */}
          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
            {ASSIGNABLE_ROLES.map((r) => {
              const isSelected = selectedRoleForPerms === r;
              return (
                <button
                  key={r}
                  onClick={() => {
                    setSelectedRoleForPerms(r);
                    setOverrides({});
                  }}
                  className={cn(
                    "flex flex-col justify-between rounded-3xl border p-5 text-left transition",
                    isSelected
                      ? "border-slate-900 bg-slate-900 text-white shadow-md ring-2 ring-slate-900/10"
                      : "border-slate-200 bg-white text-slate-800 hover:border-slate-300 hover:shadow-sm",
                  )}
                >
                  <div>
                    <div className="flex items-center justify-between">
                      <span className={cn("text-xs font-bold uppercase tracking-wider", isSelected ? "text-slate-300" : "text-slate-400")}>
                        {userCountsByRole[r]} {userCountsByRole[r] === 1 ? "usuario" : "usuarios"}
                      </span>
                      {r === "DUENO" ? (
                        <Lock className={cn("size-4", isSelected ? "text-amber-400" : "text-amber-500")} />
                      ) : (
                        <Shield className={cn("size-4", isSelected ? "text-slate-300" : "text-slate-400")} />
                      )}
                    </div>
                    <div className="mt-2 text-lg font-bold">{ROLE_LABELS[r]}</div>
                    <p className={cn("mt-1 text-xs leading-relaxed", isSelected ? "text-slate-300" : "text-slate-500")}>
                      {ROLE_DESCRIPTIONS[r]}
                    </p>
                  </div>
                  <div className="mt-4 flex items-center gap-1.5 text-xs font-semibold">
                    {isSelected ? "Editando permisos ↓" : "Configurar permisos →"}
                  </div>
                </button>
              );
            })}
          </div>

          {/* Permissions Matrix */}
          <div className="rounded-3xl border border-slate-200 bg-white p-6 shadow-sm">
            <div className="flex flex-wrap items-center justify-between gap-4 border-b border-slate-100 pb-5">
              <div>
                <h3 className="text-xl font-bold text-slate-900">
                  Permisos para el rol: <span className="text-emerald-700">{ROLE_LABELS[selectedRoleForPerms]}</span>
                </h3>
                <p className="mt-1 text-sm text-slate-500">
                  {selectedRoleForPerms === "DUENO"
                    ? "El rol Dueño tiene acceso irrestricto y permanente a todas las operaciones del sistema."
                    : "Activá o desactivá capacidades específicas para este rol en tu negocio."}
                </p>
              </div>

              {selectedRoleForPerms !== "DUENO" && (
                <button
                  onClick={handleSaveRolePermissions}
                  disabled={savingPerms}
                  className="flex h-10 items-center gap-2 rounded-full bg-slate-900 px-6 text-sm font-semibold text-white shadow-sm transition hover:bg-slate-800 disabled:opacity-50"
                >
                  {savingPerms && <RefreshCw className="size-4 animate-spin" />}
                  Guardar cambios
                </button>
              )}
            </div>

            <div className="mt-6 grid gap-6 md:grid-cols-2 lg:grid-cols-3">
              {Array.from(permissionsByModule.entries()).map(([mod, perms]) => (
                <div key={mod} className="rounded-2xl border border-slate-100 bg-slate-50/50 p-5">
                  <h4 className="font-bold text-slate-900 text-sm tracking-tight mb-3">
                    {MODULE_LABELS[mod] ?? mod.toUpperCase()}
                  </h4>
                  <div className="space-y-3">
                    {perms.map((p) => {
                      const checked =
                        selectedRoleForPerms === "DUENO" ? true : (overrides[p.code] ?? p.allowed);
                      return (
                        <label
                          key={p.code}
                          className={cn(
                            "flex items-start gap-3 rounded-xl p-2 transition",
                            selectedRoleForPerms === "DUENO"
                              ? "cursor-not-allowed opacity-90"
                              : "cursor-pointer hover:bg-white",
                          )}
                        >
                          <input
                            type="checkbox"
                            disabled={selectedRoleForPerms === "DUENO"}
                            checked={checked}
                            onChange={(e) => {
                              setOverrides((prev) => ({
                                ...prev,
                                [p.code]: e.target.checked,
                              }));
                            }}
                            className="mt-0.5 size-4 rounded border-slate-300 text-slate-900 focus:ring-slate-900"
                          />
                          <div className="text-xs">
                            <span className="font-medium text-slate-900 block">{p.description}</span>
                            <span className="text-[11px] font-mono text-slate-400">{p.code}</span>
                          </div>
                        </label>
                      );
                    })}
                  </div>
                </div>
              ))}
            </div>
          </div>
        </div>
      )}

      {/* Modal: Invitar usuario */}
      {inviteModalOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/50 p-4 backdrop-blur-xs">
          <div className="w-full max-w-lg rounded-3xl bg-white p-6 shadow-2xl sm:p-8">
            <div className="flex items-center justify-between border-b border-slate-100 pb-4">
              <div>
                <h3 className="text-xl font-bold text-slate-900">
                  {createdInvite ? "Invitación creada" : "Invitar usuario"}
                </h3>
                <p className="mt-0.5 text-xs text-slate-500">
                  {createdInvite
                    ? "Compartile este mensaje al invitado para que configure su acceso:"
                    : "Enviaremos un enlace para que cree su contraseña y comience a trabajar."}
                </p>
              </div>
              <button
                onClick={() => {
                  setInviteModalOpen(false);
                  setCreatedInvite(null);
                }}
                className="rounded-full p-2 text-slate-400 hover:bg-slate-100 hover:text-slate-600"
              >
                <X className="size-5" />
              </button>
            </div>

            {createdInvite ? (
              <div className="mt-5 space-y-4">
                <div className="rounded-2xl border border-emerald-200 bg-emerald-50 p-3 text-xs font-medium text-emerald-800">
                  ¡Invitación creada con éxito! Podés enviarle el siguiente mensaje por WhatsApp o copiarlo al portapapeles:
                </div>
                <textarea
                  readOnly
                  rows={5}
                  value={createdInvite.message}
                  className="w-full rounded-2xl border border-slate-200 bg-slate-50 p-3 text-xs text-slate-800 outline-none select-all font-mono"
                />
                <div className="flex flex-wrap items-center justify-end gap-2 pt-2">
                  <button
                    type="button"
                    onClick={() => {
                      navigator.clipboard.writeText(createdInvite.message);
                      setCopiedMessage(true);
                      setTimeout(() => setCopiedMessage(false), 2000);
                    }}
                    className="flex h-10 items-center gap-1.5 rounded-full border border-slate-200 bg-white px-4 text-xs font-semibold text-slate-700 hover:bg-slate-50"
                  >
                    {copiedMessage ? <Check className="size-4 text-emerald-600" /> : <Copy className="size-4" />}
                    {copiedMessage ? "Copiado" : "Copiar mensaje"}
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      window.open("https://wa.me/?text=" + encodeURIComponent(createdInvite.message), "_blank");
                    }}
                    className="flex h-10 items-center gap-1.5 rounded-full bg-emerald-600 px-4 text-xs font-semibold text-white shadow-xs hover:bg-emerald-700"
                  >
                    Enviar por WhatsApp
                  </button>
                  <button
                    type="button"
                    onClick={() => {
                      setInviteModalOpen(false);
                      setCreatedInvite(null);
                      reload();
                    }}
                    className="flex h-10 items-center rounded-full bg-slate-900 px-5 text-xs font-semibold text-white shadow-xs hover:bg-slate-800"
                  >
                    Listo
                  </button>
                </div>
              </div>
            ) : (
              <form onSubmit={handleSendInvite} className="mt-5 space-y-4">
                {inviteError && (
                  <div className="rounded-2xl border border-rose-200 bg-rose-50 p-3 text-xs font-medium text-rose-700">
                    {inviteError}
                  </div>
                )}

                <div>
                  <label className="block text-xs font-semibold text-slate-700 mb-1">Nombre</label>
                  <input
                    type="text"
                    required
                    placeholder="Nombre completo"
                    value={inviteName}
                    onChange={(e) => setInviteName(e.target.value)}
                    className="h-10 w-full rounded-2xl border border-slate-200 bg-white px-3.5 text-sm outline-none focus:border-slate-900 focus:ring-1 focus:ring-slate-900"
                  />
                </div>

                <div>
                  <label className="block text-xs font-semibold text-slate-700 mb-1">Correo electrónico</label>
                  <input
                    type="email"
                    required
                    placeholder="ejemplo@negocio.com"
                    value={inviteEmail}
                    onChange={(e) => setInviteEmail(e.target.value)}
                    className="h-10 w-full rounded-2xl border border-slate-200 bg-white px-3.5 text-sm outline-none focus:border-slate-900 focus:ring-1 focus:ring-slate-900"
                  />
                </div>

                <div>
                  <label className="block text-xs font-semibold text-slate-700 mb-2">Rol en el equipo</label>
                  <div className="grid max-h-56 gap-2 overflow-y-auto pr-1">
                    {availableRoles.map((r) => (
                      <label
                        key={r}
                        className={cn(
                          "flex cursor-pointer items-start gap-3 rounded-2xl border p-3 transition",
                          inviteRole === r
                            ? "border-slate-900 bg-slate-50 ring-1 ring-slate-900"
                            : "border-slate-200 hover:border-slate-300",
                        )}
                      >
                        <input
                          type="radio"
                          name="inviteRole"
                          value={r}
                          checked={inviteRole === r}
                          onChange={() => setInviteRole(r)}
                          className="mt-0.5 text-slate-900 focus:ring-slate-900"
                        />
                        <div className="text-xs">
                          <div className="font-bold text-slate-900">{ROLE_LABELS[r]}</div>
                          <div className="text-slate-500">{ROLE_DESCRIPTIONS[r]}</div>
                        </div>
                      </label>
                    ))}
                  </div>
                </div>

                <div>
                  <label className="block text-xs font-semibold text-slate-700 mb-1">Sucursal asignada</label>
                  <select
                    value={inviteBranch}
                    onChange={(e) => setInviteBranch(e.target.value)}
                    className="h-10 w-full rounded-2xl border border-slate-200 bg-white px-3.5 text-sm outline-none focus:border-slate-900"
                  >
                    <option value="ALL">Todas las sucursales</option>
                    {branches.map((b) => (
                      <option key={b.id} value={b.id}>
                        {b.name}
                      </option>
                    ))}
                  </select>
                </div>

                <div className="mt-6 flex items-center justify-end gap-3 pt-3">
                  <button
                    type="button"
                    onClick={() => setInviteModalOpen(false)}
                    className="h-10 rounded-full border border-slate-200 px-5 text-sm font-semibold text-slate-700 hover:bg-slate-50"
                  >
                    Cancelar
                  </button>
                  <button
                    type="submit"
                    disabled={submittingInvite}
                    className="flex h-10 items-center gap-2 rounded-full bg-slate-900 px-6 text-sm font-semibold text-white shadow-sm hover:bg-slate-800 disabled:opacity-50"
                  >
                    {submittingInvite && <RefreshCw className="size-4 animate-spin" />}
                    Enviar invitación
                  </button>
                </div>
              </form>
            )}
          </div>
        </div>
      )}

      {/* Modal: Cambiar rol */}
      {changeRoleUser && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/50 p-4 backdrop-blur-xs">
          <div className="w-full max-w-md rounded-3xl bg-white p-6 shadow-2xl">
            <div className="flex items-center justify-between border-b border-slate-100 pb-3">
              <h3 className="text-lg font-bold text-slate-900">Cambiar rol de usuario</h3>
              <button
                onClick={() => setChangeRoleUser(null)}
                className="rounded-full p-2 text-slate-400 hover:bg-slate-100 hover:text-slate-600"
              >
                <X className="size-4" />
              </button>
            </div>

            <div className="mt-4">
              <div className="font-semibold text-slate-900">{changeRoleUser.full_name}</div>
              <div className="text-xs text-slate-500">{changeRoleUser.email}</div>
            </div>

            <div className="mt-5 space-y-2">
              <label className="block text-xs font-semibold text-slate-700">Seleccionar nuevo rol</label>
              {availableRoles.map((r) => (
                <button
                  key={r}
                  onClick={() => handleChangeRole(changeRoleUser, r)}
                  className={cn(
                    "flex w-full items-center justify-between rounded-2xl border p-3 text-left text-xs transition",
                    changeRoleUser.role === r
                      ? "border-slate-900 bg-slate-900 text-white font-bold"
                      : "border-slate-200 hover:bg-slate-50 text-slate-800",
                  )}
                >
                  <span>{ROLE_LABELS[r]}</span>
                  {changeRoleUser.role === r && <Check className="size-4" />}
                </button>
              ))}
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

