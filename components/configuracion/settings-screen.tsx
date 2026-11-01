"use client";

import { useState } from "react";
import Link from "next/link";
import {
  Building2,
  CreditCard,
  History,
  Package,
  Printer,
  Receipt,
  RefreshCw,
  Save,
  Store,
  Users,
} from "lucide-react";

import { api, type BranchSettings } from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { mockBranchesList, mockBusinessInfo } from "@/lib/api/mock-platform";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { parseCOP } from "@/lib/format";
import { MoneyInput } from "@/components/caja/money-input";
import { cn } from "@/lib/utils";

type SettingsTab = "negocio" | "sucursales" | "pos" | "caja" | "inventario" | "impresion";

export function SettingsScreen() {
  const currentBranch = useCurrentBranch();
  const branches = useSession((s) => s.branches);

  const [activeTab, setActiveTab] = useState<SettingsTab>("pos");
  const [selectedBranchId, setSelectedBranchId] = useState<string>(currentBranch.id);
  const [toast, setToast] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  // Business info form
  const [bizInfo, setBizInfo] = useState(mockBusinessInfo);

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
                  <span className="font-bold text-slate-900 text-sm block">Un solo turno por usuario</span>
                  <span className="text-xs text-slate-500 block mt-0.5">
                    Evita que un cajero tenga múltiples sesiones o gavetas abiertas al mismo tiempo en esta sucursal.
                  </span>
                </div>
                <input
                  type="checkbox"
                  checked={formSettings.cash.one_session_per_user}
                  onChange={(e) =>
                    setFormSettings({
                      ...formSettings,
                      cash: { ...formSettings.cash, one_session_per_user: e.target.checked },
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
          <div className="grid gap-6 md:grid-cols-2">
            {mockBranchesList.map((b) => (
              <div key={b.id} className="rounded-3xl border border-slate-200 bg-white p-6 shadow-sm">
                <div className="flex items-start justify-between">
                  <div>
                    <h3 className="text-xl font-bold text-slate-900">{b.name}</h3>
                    <p className="text-xs text-slate-500 mt-0.5">{b.city} · Código: {b.code}</p>
                  </div>
                  <span className="rounded-full bg-slate-100 px-3 py-1 text-xs font-semibold text-slate-700">
                    {b.userCount} usuarios
                  </span>
                </div>

                <div className="mt-4 flex flex-wrap gap-1.5">
                  {b.activeTypes.map((t) => (
                    <span key={t} className="rounded-full bg-emerald-50 border border-emerald-200 px-2.5 py-0.5 text-xs font-medium text-emerald-700">
                      {t}
                    </span>
                  ))}
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
                value={bizInfo.commercial_name}
                onChange={(e) => setBizInfo({ ...bizInfo, commercial_name: e.target.value })}
                className="h-10 w-full rounded-2xl border border-slate-200 bg-white px-3.5 text-sm outline-none focus:border-slate-900"
              />
            </div>

            <div>
              <label className="block text-xs font-semibold text-slate-700 mb-1">Razón social</label>
              <input
                type="text"
                value={bizInfo.legal_name}
                onChange={(e) => setBizInfo({ ...bizInfo, legal_name: e.target.value })}
                className="h-10 w-full rounded-2xl border border-slate-200 bg-white px-3.5 text-sm outline-none focus:border-slate-900"
              />
            </div>

            <div className="grid grid-cols-2 gap-4">
              <div>
                <label className="block text-xs font-semibold text-slate-700 mb-1">NIT / Documento</label>
                <input
                  type="text"
                  value={bizInfo.tax_id}
                  onChange={(e) => setBizInfo({ ...bizInfo, tax_id: e.target.value })}
                  className="h-10 w-full rounded-2xl border border-slate-200 bg-white px-3.5 text-sm outline-none focus:border-slate-900"
                />
              </div>
              <div>
                <label className="block text-xs font-semibold text-slate-700 mb-1">Teléfono</label>
                <input
                  type="text"
                  value={bizInfo.phone}
                  onChange={(e) => setBizInfo({ ...bizInfo, phone: e.target.value })}
                  className="h-10 w-full rounded-2xl border border-slate-200 bg-white px-3.5 text-sm outline-none focus:border-slate-900"
                />
              </div>
            </div>

            <div>
              <label className="block text-xs font-semibold text-slate-700 mb-1">Correo de facturación</label>
              <input
                type="email"
                value={bizInfo.email}
                onChange={(e) => setBizInfo({ ...bizInfo, email: e.target.value })}
                className="h-10 w-full rounded-2xl border border-slate-200 bg-white px-3.5 text-sm outline-none focus:border-slate-900"
              />
            </div>

            <div className="pt-4 flex justify-end">
              <button
                type="button"
                onClick={() => notify("Información de la empresa actualizada")}
                className="flex h-10 items-center gap-2 rounded-full bg-slate-900 px-6 text-sm font-semibold text-white shadow-xs transition hover:bg-slate-800"
              >
                <Save className="size-4" />
                Guardar cambios
              </button>
            </div>
          </div>
        </div>
      )}
    </div>
  );
}

