"use client";

import { useState } from "react";
import Link from "next/link";
import {
  Building2,
  Calculator,
  CreditCard,
  History,
  Package,
  Printer,
  Receipt,
  RefreshCw,
  Save,
  Store,
  Users,
  X,
  QrCode,
} from "lucide-react";

import { api, type BranchSettings, friendlyError } from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { parseCOP } from "@/lib/format";
import { MoneyInput } from "@/components/caja/money-input";
import { cn } from "@/lib/utils";
import { nitDv } from "@/lib/nit";

type SettingsTab = "negocio" | "sucursales" | "pos" | "caja" | "inventario" | "impresion";

export function SettingsScreen() {
  const currentBranch = useCurrentBranch();
  const branches = useSession((s) => s.branches);
  const can = useSession((s) => s.can);
  const boot = useSession((s) => s.boot);
  const isOwner = branches.some((b) => b.role === "DUENO");

  const [activeTab, setActiveTab] = useState<SettingsTab>("pos");
  const [selectedBranchId, setSelectedBranchId] = useState<string>(currentBranch.id);
  const [toast, setToast] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [savingBiz, setSavingBiz] = useState(false);

  // Business info form
  const { data: businessData, reload: reloadBusiness } = useApi(
    () => api.business.get(),
    "business-info",
  );

  const [bizDraft, setBizDraft] = useState<{
    name?: string;
    business_name?: string | null;
    nit?: string | null;
    iva_responsible?: boolean | null;
  }>({});

  const bizName = bizDraft.name ?? businessData?.name ?? "";
  const bizLegalName =
    bizDraft.business_name !== undefined
      ? (bizDraft.business_name ?? "")
      : (businessData?.business_name ?? "");
  const bizNit =
    bizDraft.nit !== undefined ? (bizDraft.nit ?? "") : (businessData?.nit ?? "");
  const bizIva =
    bizDraft.iva_responsible !== undefined
      ? bizDraft.iva_responsible
      : (businessData?.iva_responsible ?? null);

  const computedDv = nitDv(bizNit);

  // Branch management states
  const [editBranch, setEditBranch] = useState<{ id: string; name: string } | null>(null);
  const [editBranchName, setEditBranchName] = useState("");
  const [savingEditBranch, setSavingEditBranch] = useState(false);

  const [newBranchModalOpen, setNewBranchModalOpen] = useState(false);
  const [newBranchName, setNewBranchName] = useState("");
  const [savingNewBranch, setSavingNewBranch] = useState(false);

  // Branch settings
  const { data: settingsData, reload } = useApi(
    () => api.settings.get(selectedBranchId),
    `branch-settings-${selectedBranchId}`,
  );

  const [settingsDraft, setSettingsDraft] = useState<BranchSettings | null>(null);
  const formSettings = settingsDraft ?? settingsData ?? null;

  function setFormSettings(updated: BranchSettings) {
    setSettingsDraft(updated);
  }

  function notify(msg: string) {
    setToast(msg);
    setTimeout(() => setToast(null), 4000);
  }

  async function handleSaveSettings() {
    if (!formSettings) return;
    setSaving(true);
    try {
      await api.settings.update(selectedBranchId, formSettings);
      setSettingsDraft(null);
      notify("Ajustes guardados correctamente");
      reload();
    } catch {
      notify("No se pudieron guardar los ajustes");
    } finally {
      setSaving(false);
    }
  }

  return (
    <div className="space-y-6 p-6 lg:p-10">
      {/* Toast */}
      {toast && (
        <div className="fixed top-6 right-6 z-50 rounded-2xl bg-slate-900 px-5 py-3 text-sm font-medium text-white shadow-xl transition-all">
          {toast}
        </div>
      )}

      {/* Header */}
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Configuración del negocio</h1>
          <p className="mt-1 text-slate-500">
            Ajustes generales, sucursales y reglas operativas de venta, caja e inventario.
          </p>
        </div>

        {/* Shortcuts */}
        <div className="flex flex-wrap items-center gap-2">
          {can("settings:write") && (
            <Link
              href="/configuracion/cajas"
              className="flex h-10 items-center gap-2 rounded-full border border-slate-200 bg-white px-4 text-xs font-semibold text-slate-700 shadow-xs hover:bg-slate-50"
            >
              <Calculator className="size-4" />
              Cajas registradoras
            </Link>
          )}
          <Link
            href="/configuracion/whatsapp"
            className="flex h-10 items-center gap-2 rounded-full border border-slate-200 bg-white px-4 text-xs font-semibold text-slate-700 shadow-xs hover:bg-slate-50"
          >
            <QrCode className="size-4" />
            QR de WhatsApp
          </Link>
          <Link
            href="/configuracion/usuarios"
            className="flex h-10 items-center gap-2 rounded-full border border-slate-200 bg-white px-4 text-xs font-semibold text-slate-700 shadow-xs hover:bg-slate-50"
          >
            <Users className="size-4" />
            Usuarios y roles
          </Link>
          <Link
            href="/configuracion/auditoria"
            className="flex h-10 items-center gap-2 rounded-full border border-slate-200 bg-white px-4 text-xs font-semibold text-slate-700 shadow-xs hover:bg-slate-50"
          >
            <History className="size-4" />
            Registro de auditoría
          </Link>
        </div>
      </div>

      {/* Tabs */}
      <div className="flex flex-wrap gap-2 border-b border-slate-200 pb-3">
        {[
          { id: "pos", label: "Punto de Venta (POS)", icon: Receipt },
          { id: "caja", label: "Caja y Arqueo", icon: CreditCard },
          { id: "inventario", label: "Inventario y Stock", icon: Package },
          { id: "impresion", label: "Impresión y Tirilla", icon: Printer },
          { id: "sucursales", label: "Sucursales", icon: Store },
          { id: "negocio", label: "Datos del Negocio", icon: Building2 },
        ].map((tab) => {
          const Icon = tab.icon;
          const isActive = activeTab === tab.id;
          return (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id as SettingsTab)}
              className={cn(
                "flex items-center gap-2 rounded-full px-4 py-2 text-sm font-medium transition",
                isActive
                  ? "bg-slate-900 text-white shadow-xs"
                  : "bg-white text-slate-600 border border-slate-200 hover:text-slate-900 hover:bg-slate-50",
              )}
            >
              <Icon className="size-4" />
              {tab.label}
            </button>
          );
        })}
      </div>

      {/* Branch selector for branch-specific tabs */}
      {["pos", "caja", "inventario", "impresion"].includes(activeTab) && (
        <div className="flex items-center justify-between rounded-2xl bg-slate-100/70 p-3 px-5">
          <div className="flex items-center gap-3">
            <Store className="size-5 text-slate-600" />
            <span className="text-sm font-semibold text-slate-800">Configurando sucursal:</span>
            <select
              value={selectedBranchId}
              onChange={(e) => {
                setSelectedBranchId(e.target.value);
                setSettingsDraft(null);
              }}
              className="rounded-xl border border-slate-300 bg-white px-3 py-1.5 text-sm font-medium text-slate-900 outline-none focus:border-slate-900"
            >
              {branches.map((b) => (
                <option key={b.id} value={b.id}>
                  {b.name}
                </option>
              ))}
            </select>
          </div>

          <button
            onClick={handleSaveSettings}
            disabled={saving}
            className="flex h-9 items-center gap-2 rounded-full bg-slate-900 px-5 text-xs font-semibold text-white shadow-xs transition hover:bg-slate-800 disabled:opacity-50"
          >
            {saving ? <RefreshCw className="size-3.5 animate-spin" /> : <Save className="size-3.5" />}
            Guardar cambios
          </button>
        </div>
      )}

      {/* Tab: Punto de Venta (POS) */}
      {activeTab === "pos" && formSettings && (
        <div className="rounded-3xl border border-slate-200 bg-white p-6 shadow-sm sm:p-8">
          <h2 className="text-lg font-bold text-slate-900 mb-1">Reglas del Punto de Venta</h2>
          <p className="text-xs text-slate-500 mb-6">Políticas de cobro, descuentos y devoluciones en mostrador.</p>

          <div className="grid gap-6 md:grid-cols-2">
            <div className="rounded-2xl border border-slate-100 bg-slate-50 p-5">
              <label className="flex items-start justify-between gap-4 cursor-pointer">
                <div>
                  <span className="font-bold text-slate-900 text-sm block">Permitir modificación de precios</span>
                  <span className="text-xs text-slate-500 block mt-0.5">
                    Permite al cajero cambiar el precio unitario en el mostrador al agregar productos.
                  </span>
                </div>
                <input
                  type="checkbox"
                  checked={formSettings.pos.allow_price_override}
                  onChange={(e) =>
                    setFormSettings({
                      ...formSettings,
                      pos: { ...formSettings.pos, allow_price_override: e.target.checked },
                    })
                  }
                  className="mt-1 size-5 rounded border-slate-300 text-slate-900 focus:ring-slate-900"
                />
              </label>
            </div>

            <div className="rounded-2xl border border-slate-100 bg-slate-50 p-5">
              <label className="flex items-start justify-between gap-4 cursor-pointer">
                <div>
                  <span className="font-bold text-slate-900 text-sm block">Devoluciones requieren aprobación</span>
                  <span className="text-xs text-slate-500 block mt-0.5">
                    Exige autorización de un administrador para cancelar una orden en preparación o cobrar notas crédito.
                  </span>
                </div>
                <input
                  type="checkbox"
                  checked={formSettings.pos.refund_requires_approval}
                  onChange={(e) =>
                    setFormSettings({
                      ...formSettings,
                      pos: { ...formSettings.pos, refund_requires_approval: e.target.checked },
                    })
                  }
                  className="mt-1 size-5 rounded border-slate-300 text-slate-900 focus:ring-slate-900"
                />
              </label>
            </div>

            <div className="rounded-2xl border border-slate-100 bg-slate-50 p-5">
              <div>
                <span className="font-bold text-slate-900 text-sm block">Porcentaje máximo de descuento (%)</span>
                <span className="text-xs text-slate-500 block mt-0.5">
                  Límite máximo de descuento que puede aplicar un cajero por línea de producto.
                </span>
                <div className="mt-3 flex items-center gap-2">
                  <input
                    type="number"
                    min="0"
                    max="100"
                    value={formSettings.pos.max_discount_pct}
                    onChange={(e) =>
                      setFormSettings({
                        ...formSettings,
                        pos: { ...formSettings.pos, max_discount_pct: Number(e.target.value) },
                      })
                    }
                    className="h-10 w-28 rounded-xl border border-slate-300 bg-white px-3 text-sm font-semibold outline-none focus:border-slate-900"
                  />
                  <span className="text-sm font-medium text-slate-500">% de descuento máximo</span>
                </div>
              </div>
            </div>

            <div className="rounded-2xl border border-slate-100 bg-slate-50 p-5">
              <label className="flex items-start justify-between gap-4 cursor-pointer">
                <div>
                  <span className="font-bold text-slate-900 text-sm block">Ventas anónimas permitidas</span>
                  <span className="text-xs text-slate-500 block mt-0.5">
                    Permite facturar a &quot;Consumidor Final&quot; sin exigir cédula ni datos del cliente en montos menores.
                  </span>
                </div>
                <input
                  type="checkbox"
                  checked={formSettings.pos.allow_anonymous_sales ?? true}
                  onChange={(e) =>
                    setFormSettings({
                      ...formSettings,
                      pos: { ...formSettings.pos, allow_anonymous_sales: e.target.checked },
                    })
                  }
                  className="mt-1 size-5 rounded border-slate-300 text-slate-900 focus:ring-slate-900"
                />
              </label>
            </div>
          </div>
        </div>
      )}

      {/* Tab: Caja y Arqueo */}
      {activeTab === "caja" && formSettings && (
        <div className="rounded-3xl border border-slate-200 bg-white p-6 shadow-sm sm:p-8">
          <h2 className="text-lg font-bold text-slate-900 mb-1">Ajustes de Caja y Turnos</h2>
          <p className="text-xs text-slate-500 mb-6">Políticas de arqueo, tolerancia de descuadres y control de turnos.</p>

          <div className="grid gap-6 md:grid-cols-2">
            <div className="rounded-2xl border border-slate-100 bg-slate-50 p-5">
              <label className="flex items-start justify-between gap-4 cursor-pointer">
                <div>
                  <span className="font-bold text-slate-900 text-sm block">Exigir turno de caja para vender</span>
                  <span className="text-xs text-slate-500 block mt-0.5">
                    Impide procesar ventas si el cajero no ha registrado su apertura de turno con base de efectivo.
                  </span>
                </div>
                <input
                  type="checkbox"
                  checked={formSettings.cash.require_session}
                  onChange={(e) =>
                    setFormSettings({
                      ...formSettings,
                      cash: { ...formSettings.cash, require_session: e.target.checked },
                    })
                  }
                  className="mt-1 size-5 rounded border-slate-300 text-slate-900 focus:ring-slate-900"
                />
              </label>
            </div>


            <div className="rounded-2xl border border-slate-100 bg-slate-50 p-5">
              <label className="flex items-start justify-between gap-4 cursor-pointer">
                <div>
                  <span className="font-bold text-slate-900 text-sm block">Cierre a ciegas (Blind close)</span>
                  <span className="text-xs text-slate-500 block mt-0.5">
                    Oculta el total esperado en gaveta al cajero durante el conteo para garantizar arqueos transparentes.
                  </span>
                </div>
                <input
                  type="checkbox"
                  checked={formSettings.cash.blind_close ?? true}
                  onChange={(e) =>
                    setFormSettings({
                      ...formSettings,
                      cash: { ...formSettings.cash, blind_close: e.target.checked },
                    })
                  }
                  className="mt-1 size-5 rounded border-slate-300 text-slate-900 focus:ring-slate-900"
                />
              </label>
            </div>

            <div className="rounded-2xl border border-slate-100 bg-slate-50 p-5">
              <div>
                <span className="font-bold text-slate-900 text-sm block">Tolerancia máxima de descuadre</span>
                <span className="text-xs text-slate-500 block mt-0.5">
                  Si la diferencia entre el efectivo contado y el esperado supera este monto, es obligatorio escribir una nota de justificación.
                </span>
                <div className="mt-3 w-48">
                  <MoneyInput
                    value={parseCOP(formSettings.cash.close_tolerance)}
                    onChange={(val) =>
                      setFormSettings({
                        ...formSettings,
                        cash: { ...formSettings.cash, close_tolerance: val.toFixed(2) },
                      })
                    }
                  />
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Tab: Inventario */}
      {activeTab === "inventario" && formSettings && (
        <div className="rounded-3xl border border-slate-200 bg-white p-6 shadow-sm sm:p-8">
          <h2 className="text-lg font-bold text-slate-900 mb-1">Reglas de Inventario y Existencias</h2>
          <p className="text-xs text-slate-500 mb-6">Control de negativos, insumos de recetas y alertas de stock.</p>

          <div className="grid gap-6 md:grid-cols-2">
            <div className="rounded-2xl border border-slate-100 bg-slate-50 p-5">
              <label className="flex items-start justify-between gap-4 cursor-pointer">
                <div>
                  <span className="font-bold text-slate-900 text-sm block">Permitir stock en negativo</span>
                  <span className="text-xs text-slate-500 block mt-0.5">
                    Si está desactivado, el sistema rechazará cualquier venta o traslado si no hay existencias físicas registradas.
                  </span>
                </div>
                <input
                  type="checkbox"
                  checked={formSettings.inventory.allow_negative_stock}
                  onChange={(e) =>
                    setFormSettings({
                      ...formSettings,
                      inventory: { ...formSettings.inventory, allow_negative_stock: e.target.checked },
                    })
                  }
                  className="mt-1 size-5 rounded border-slate-300 text-slate-900 focus:ring-slate-900"
                />
              </label>
            </div>

            <div className="rounded-2xl border border-slate-100 bg-slate-50 p-5">
              <label className="flex items-start justify-between gap-4 cursor-pointer">
                <div>
                  <span className="font-bold text-slate-900 text-sm block">Descontar insumos de recetas (Materia prima)</span>
                  <span className="text-xs text-slate-500 block mt-0.5">
                    Descuenta automáticamente los ingredientes o insumos al confirmar órdenes o comandas de platos preparados.
                  </span>
                </div>
                <input
                  type="checkbox"
                  checked={formSettings.inventory.track_raw_materials}
                  onChange={(e) =>
                    setFormSettings({
                      ...formSettings,
                      inventory: { ...formSettings.inventory, track_raw_materials: e.target.checked },
                    })
                  }
                  className="mt-1 size-5 rounded border-slate-300 text-slate-900 focus:ring-slate-900"
                />
              </label>
            </div>

            <div className="rounded-2xl border border-slate-100 bg-slate-50 p-5 md:col-span-2">
              <div>
                <span className="font-bold text-slate-900 text-sm block">Método de costeo de inventario</span>
                <span className="text-xs text-slate-500 block mt-0.5 mb-3">
                  Define cómo se calcula el costo de la mercancía vendida (CMV) al registrar ventas y salidas de stock.
                </span>
                <div className="grid gap-3 sm:grid-cols-2">
                  {[
                    { value: "PROMEDIO", label: "Costo Promedio Ponderado", desc: "Recalcula el costo unitario con cada entrada y valora salidas al costo medio actual." },
                    { value: "PEPS", label: "PEPS / FIFO", desc: "Primero en Entrar, Primero en Salir. Consume las capas de costo más antiguas primero." },
                  ].map((opt) => (
                    <button
                      key={opt.value}
                      type="button"
                      onClick={() =>
                        setFormSettings({
                          ...formSettings,
                          inventory: {
                            ...formSettings.inventory,
                            costing: opt.value as "PROMEDIO" | "PEPS",
                          },
                        })
                      }
                      className={`rounded-xl border p-4 text-left transition-all ${
                        (formSettings.inventory.costing || "PROMEDIO") === opt.value
                          ? "border-slate-900 bg-white ring-2 ring-slate-900/10 shadow-sm"
                          : "border-slate-200 bg-white hover:border-slate-300"
                      }`}
                    >
                      <span className="font-bold text-slate-900 text-sm block">{opt.label}</span>
                      <span className="text-xs text-slate-500 block mt-1">{opt.desc}</span>
                    </button>
                  ))}
                </div>
              </div>
            </div>
          </div>
        </div>
      )}

      {/* Tab: Impresión y Tirilla */}
      {activeTab === "impresion" && formSettings && (
        <div className="rounded-3xl border border-slate-200 bg-white p-6 shadow-sm sm:p-8">
          <h2 className="text-lg font-bold text-slate-900 mb-1">Configuración de Comprobantes e Impresión</h2>
          <p className="text-xs text-slate-500 mb-6">Formato y textos impresos en la tirilla térmica para clientes.</p>

          <div className="space-y-6 max-w-2xl">
            <div>
              <label className="block text-xs font-semibold text-slate-700 mb-2">Ancho de papel de la impresora</label>
              <div className="flex gap-4">
                {[80, 58].map((w) => (
                  <button
                    key={w}
                    type="button"
                    onClick={() =>
                      setFormSettings({
                        ...formSettings,
                        print: { ...formSettings.print, ticket_width_mm: w },
                      })
                    }
                    className={cn(
                      "flex items-center gap-2 rounded-2xl border px-5 py-3 text-sm font-bold transition",
                      formSettings.print.ticket_width_mm === w
                        ? "border-slate-900 bg-slate-900 text-white"
                        : "border-slate-200 bg-white text-slate-700 hover:border-slate-300",
                    )}
                  >
                    <Printer className="size-4" />
                    {w} mm {w === 80 ? "(Estándar POS)" : "(Mini térmica)"}
                  </button>
                ))}
              </div>
            </div>

            <div>
              <label className="block text-xs font-semibold text-slate-700 mb-1">Encabezado de la tirilla</label>
              <textarea
                rows={3}
                value={formSettings.print.header_text ?? ""}
                onChange={(e) =>
                  setFormSettings({
                    ...formSettings,
                    print: { ...formSettings.print, header_text: e.target.value },
                  })
                }
                placeholder="Nombre comercial, NIT, Régimen, Dirección..."
                className="w-full rounded-2xl border border-slate-200 bg-white p-3 text-xs outline-none focus:border-slate-900 font-mono"
              />
            </div>

            <div>
              <label className="block text-xs font-semibold text-slate-700 mb-1">Pie de página de la tirilla</label>
              <textarea
                rows={3}
                value={formSettings.print.footer_text ?? ""}
                onChange={(e) =>
                  setFormSettings({
                    ...formSettings,
                    print: { ...formSettings.print, footer_text: e.target.value },
                  })
                }
                placeholder="Mensaje de agradecimiento, redes sociales, políticas de garantía..."
                className="w-full rounded-2xl border border-slate-200 bg-white p-3 text-xs outline-none focus:border-slate-900 font-mono"
              />
            </div>
          </div>
        </div>
      )}

      {/* Tab: Sucursales */}
      {activeTab === "sucursales" && (
        <div className="space-y-6">
          <div className="flex items-center justify-between">
            <div>
              <h2 className="text-lg font-bold text-slate-900">Sucursales del Negocio</h2>
              <p className="text-xs text-slate-500">Puntos de venta habilitados para tu cuenta.</p>
            </div>
            {isOwner && (
              <button
                type="button"
                onClick={() => {
                  setNewBranchName("");
                  setNewBranchModalOpen(true);
                }}
                className="flex h-10 items-center gap-2 rounded-full bg-slate-900 px-5 text-xs font-semibold text-white shadow-xs hover:bg-slate-800"
              >
                <Store className="size-4" />
                Nueva sucursal
              </button>
            )}
          </div>

          <div className="grid gap-6 md:grid-cols-2">
            {branches.map((b) => (
              <div key={b.id} className="rounded-3xl border border-slate-200 bg-white p-6 shadow-sm">
                <div className="flex items-start justify-between">
                  <div>
                    <h3 className="text-xl font-bold text-slate-900">{b.name}</h3>
                    <p className="text-xs text-slate-500 mt-0.5">
                      Rol: {b.role}{b.code ? ` · Código: ${b.code}` : ""}
                    </p>
                  </div>
                  <button
                    type="button"
                    onClick={() => {
                      setEditBranch({ id: b.id, name: b.name });
                      setEditBranchName(b.name);
                    }}
                    className="rounded-full border border-slate-200 bg-white px-3.5 py-1.5 text-xs font-semibold text-slate-700 shadow-xs hover:bg-slate-50"
                  >
                    Cambiar nombre
                  </button>
                </div>
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Tab: Negocio */}
      {activeTab === "negocio" && (
        <div className="rounded-3xl border border-slate-200 bg-white p-6 shadow-sm sm:p-8 max-w-2xl">
          <h2 className="text-lg font-bold text-slate-900 mb-1">Datos Principales de la Empresa</h2>
          <p className="text-xs text-slate-500 mb-6">Información fiscal y comercial registrada ante la DIAN.</p>

          <div className="space-y-4">
            <div>
              <label className="block text-xs font-semibold text-slate-700 mb-1">Nombre comercial</label>
              <input
                type="text"
                value={bizName}
                onChange={(e) => setBizDraft((prev) => ({ ...prev, name: e.target.value }))}
                placeholder="Nombre comercial de la empresa"
                className="h-10 w-full rounded-2xl border border-slate-200 bg-white px-3.5 text-sm outline-none focus:border-slate-900"
              />
            </div>

            <div>
              <label className="block text-xs font-semibold text-slate-700 mb-1">Razón social</label>
              <input
                type="text"
                disabled={!isOwner}
                value={bizLegalName}
                onChange={(e) => setBizDraft((prev) => ({ ...prev, business_name: e.target.value }))}
                placeholder="Razón social registrada"
                className="h-10 w-full rounded-2xl border border-slate-200 bg-white px-3.5 text-sm outline-none focus:border-slate-900 disabled:bg-slate-50 disabled:text-slate-500 disabled:cursor-not-allowed"
              />
              {!isOwner && (
                <p className="text-xs text-slate-500 mt-1">Solo el dueño puede cambiar estos datos.</p>
              )}
            </div>

            <div className="grid grid-cols-3 gap-4">
              <div className="col-span-2">
                <label className="block text-xs font-semibold text-slate-700 mb-1">NIT</label>
                <input
                  type="text"
                  disabled={!isOwner}
                  value={bizNit}
                  onChange={(e) => setBizDraft((prev) => ({ ...prev, nit: e.target.value }))}
                  placeholder="Sin puntos ni guiones (ej. 900123456)"
                  className="h-10 w-full rounded-2xl border border-slate-200 bg-white px-3.5 text-sm outline-none focus:border-slate-900 disabled:bg-slate-50 disabled:text-slate-500 disabled:cursor-not-allowed"
                />
                {!isOwner && (
                  <p className="text-xs text-slate-500 mt-1">Solo el dueño puede cambiar estos datos.</p>
                )}
              </div>
              <div>
                <label className="block text-xs font-semibold text-slate-700 mb-1">DV</label>
                <input
                  type="text"
                  readOnly
                  disabled
                  value={computedDv !== null ? String(computedDv) : "-"}
                  className="h-10 w-full rounded-2xl border border-slate-200 bg-slate-50 px-3.5 text-center text-sm font-bold text-slate-700 outline-none cursor-not-allowed"
                />
              </div>
            </div>

            <div>
              <label className="block text-xs font-semibold text-slate-700 mb-1">¿Es responsable de IVA?</label>
              <select
                disabled={!isOwner}
                value={bizIva === null ? "" : bizIva ? "true" : "false"}
                onChange={(e) => {
                  const val = e.target.value;
                  setBizDraft((prev) => ({
                    ...prev,
                    iva_responsible: val === "" ? null : val === "true",
                  }));
                }}
                className="h-10 w-full rounded-2xl border border-slate-200 bg-white px-3.5 text-sm outline-none focus:border-slate-900 disabled:bg-slate-50 disabled:text-slate-500 disabled:cursor-not-allowed"
              >
                <option value="">Sin definir</option>
                <option value="true">Sí</option>
                <option value="false">No</option>
              </select>
              {!isOwner && (
                <p className="text-xs text-slate-500 mt-1">Solo el dueño puede cambiar estos datos.</p>
              )}
            </div>

            <div className="pt-4 flex justify-end">
              <button
                type="button"
                disabled={savingBiz}
                onClick={async () => {
                  setSavingBiz(true);
                  try {
                    await api.business.update({
                      name: bizName,
                      business_name: bizLegalName || null,
                      nit: bizNit || null,
                      dv: computedDv !== null ? String(computedDv) : null,
                      iva_responsible: bizIva,
                    });
                    setBizDraft({});
                    notify("Datos del negocio guardados");
                    reloadBusiness();
                  } catch (err) {
                    notify(friendlyError(err));
                  } finally {
                    setSavingBiz(false);
                  }
                }}
                className="flex h-10 items-center gap-2 rounded-full bg-slate-900 px-6 text-sm font-semibold text-white shadow-xs transition hover:bg-slate-800 disabled:opacity-50"
              >
                {savingBiz ? <RefreshCw className="size-4 animate-spin" /> : <Save className="size-4" />}
                Guardar datos
              </button>
            </div>
          </div>
        </div>
      )}

      {/* Modal Cambiar Nombre Sucursal */}
      {editBranch && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/40 p-4 backdrop-blur-sm">
          <div className="w-full max-w-md rounded-3xl bg-white p-6 shadow-2xl">
            <div className="flex items-center justify-between border-b border-slate-100 pb-4">
              <h2 className="text-lg font-bold text-slate-900">Cambiar nombre</h2>
              <button
                onClick={() => setEditBranch(null)}
                className="rounded-xl p-1 text-slate-400 hover:bg-slate-100 hover:text-slate-700"
              >
                <X className="size-5" />
              </button>
            </div>

            <form
              onSubmit={async (e) => {
                e.preventDefault();
                setSavingEditBranch(true);
                try {
                  await api.branches.update(editBranch.id, { name: editBranchName });
                  await boot();
                  setEditBranch(null);
                  notify("Nombre de la sucursal actualizado");
                } catch (err) {
                  notify(friendlyError(err));
                } finally {
                  setSavingEditBranch(false);
                }
              }}
              className="mt-5 space-y-4"
            >
              <div>
                <label className="text-xs font-semibold text-slate-700">Nombre de la sucursal</label>
                <input
                  type="text"
                  required
                  value={editBranchName}
                  onChange={(e) => setEditBranchName(e.target.value)}
                  placeholder="Nombre de la sucursal"
                  className="mt-1 h-10 w-full rounded-2xl border border-slate-200 bg-white px-3.5 text-sm outline-none focus:border-slate-900"
                />
              </div>

              <div className="flex justify-end gap-2 pt-2">
                <button
                  type="button"
                  onClick={() => setEditBranch(null)}
                  className="rounded-full border border-slate-200 px-4 py-2 text-xs font-semibold text-slate-700 hover:bg-slate-50"
                >
                  Cancelar
                </button>
                <button
                  type="submit"
                  disabled={savingEditBranch}
                  className="rounded-full bg-slate-900 px-5 py-2 text-xs font-semibold text-white hover:bg-slate-800 disabled:opacity-50"
                >
                  {savingEditBranch ? "Guardando..." : "Guardar"}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}

      {/* Modal Nueva Sucursal */}
      {newBranchModalOpen && (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-slate-900/40 p-4 backdrop-blur-sm">
          <div className="w-full max-w-md rounded-3xl bg-white p-6 shadow-2xl">
            <div className="flex items-center justify-between border-b border-slate-100 pb-4">
              <h2 className="text-lg font-bold text-slate-900">Nueva sucursal</h2>
              <button
                onClick={() => setNewBranchModalOpen(false)}
                className="rounded-xl p-1 text-slate-400 hover:bg-slate-100 hover:text-slate-700"
              >
                <X className="size-5" />
              </button>
            </div>

            <form
              onSubmit={async (e) => {
                e.preventDefault();
                setSavingNewBranch(true);
                try {
                  await api.branches.create({ name: newBranchName });
                  await boot();
                  setNewBranchModalOpen(false);
                  notify("Sucursal creada exitosamente");
                } catch (err) {
                  notify(friendlyError(err));
                } finally {
                  setSavingNewBranch(false);
                }
              }}
              className="mt-5 space-y-4"
            >
              <div>
                <label className="text-xs font-semibold text-slate-700">Nombre de la sucursal</label>
                <input
                  type="text"
                  required
                  value={newBranchName}
                  onChange={(e) => setNewBranchName(e.target.value)}
                  placeholder="Nombre de la sucursal"
                  className="mt-1 h-10 w-full rounded-2xl border border-slate-200 bg-white px-3.5 text-sm outline-none focus:border-slate-900"
                />
              </div>

              <div className="flex justify-end gap-2 pt-2">
                <button
                  type="button"
                  onClick={() => setNewBranchModalOpen(false)}
                  className="rounded-full border border-slate-200 px-4 py-2 text-xs font-semibold text-slate-700 hover:bg-slate-50"
                >
                  Cancelar
                </button>
                <button
                  type="submit"
                  disabled={savingNewBranch}
                  className="rounded-full bg-slate-900 px-5 py-2 text-xs font-semibold text-white hover:bg-slate-800 disabled:opacity-50"
                >
                  {savingNewBranch ? "Creando..." : "Guardar"}
                </button>
              </div>
            </form>
          </div>
        </div>
      )}
    </div>
  );
}

