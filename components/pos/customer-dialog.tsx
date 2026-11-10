"use client";

import { useEffect, useState } from "react";
import { Check, Loader2, Plus, Search, Tag, User, Users } from "lucide-react";

import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import {
  api,
  ApiError,
  friendlyError,
  type CreateCustomerInput,
  type Customer,
  type DocType,
} from "@/lib/api";
import { cn } from "@/lib/utils";

const DOC_TYPES: { code: DocType | "NONE"; label: string }[] = [
  { code: "NONE", label: "Sin documento" },
  { code: "CC", label: "Cédula de Ciudadanía (CC)" },
  { code: "NIT", label: "NIT" },
  { code: "CE", label: "Cédula de Extranjería (CE)" },
  { code: "PP", label: "Pasaporte (PP)" },
  { code: "TI", label: "Tarjeta de Identidad (TI)" },
  { code: "PEP", label: "Permiso Especial de Permanencia (PEP)" },
  { code: "PPT", label: "Permiso por Protección Temporal (PPT)" },
  { code: "NIUP", label: "NIUP" },
];

export function CustomerDialog({
  open,
  onOpenChange,
  currentCustomer,
  currentAlias,
  onSelectCustomer,
  onSelectAlias,
  allowAliasSales = true,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  currentCustomer: Customer | null;
  currentAlias: string | null;
  onSelectCustomer: (customer: Customer | null) => void;
  onSelectAlias: (alias: string | null) => void;
  allowAliasSales?: boolean;
}) {
  const [tab, setTab] = useState<"search" | "new" | "alias">(
    currentAlias ? "alias" : "search",
  );

  // Search state
  const [searchQuery, setSearchQuery] = useState("");
  const [searchResults, setSearchResults] = useState<Customer[]>([]);
  const [searching, setSearching] = useState(false);

  // New customer form state
  const [name, setName] = useState("");
  const [docType, setDocType] = useState<DocType | "NONE">("CC");
  const [docNumber, setDocNumber] = useState("");
  const [dv, setDv] = useState("");
  const [phone, setPhone] = useState("");
  const [email, setEmail] = useState("");
  const [saving, setSaving] = useState(false);
  const [formError, setFormError] = useState<string | null>(null);
  const [successMsg, setSuccessMsg] = useState<string | null>(null);
  const [duplicateId, setDuplicateId] = useState<string | null>(null);

  // Alias state
  const [alias, setAlias] = useState(currentAlias ?? "");
  const [aliasError, setAliasError] = useState<string | null>(null);

  // Debounced search (300ms)
  useEffect(() => {
    if (!open || tab !== "search") return;
    const timer = setTimeout(async () => {
      setSearching(true);
      try {
        const results = await api.customers.search({ q: searchQuery });
        setSearchResults(results);
      } catch (err) {
        console.error("Error searching customers:", err);
      } finally {
        setSearching(false);
      }
    }, 300);

    return () => clearTimeout(timer);
  }, [searchQuery, open, tab]);

  // Reset form when opening
  const [lastOpen, setLastOpen] = useState(open);
  if (open !== lastOpen) {
    setLastOpen(open);
    if (open) {
      setTab(currentAlias ? "alias" : "search");
      setSearchQuery("");
      setName("");
      setDocType("CC");
      setDocNumber("");
      setDv("");
      setPhone("");
      setEmail("");
      setFormError(null);
      setSuccessMsg(null);
      setDuplicateId(null);
      setAlias(currentAlias ?? "");
      setAliasError(null);
    }
  }

  async function handleCreate(e: React.FormEvent) {
    e.preventDefault();
    setFormError(null);
    setSuccessMsg(null);
    setDuplicateId(null);

    const trimmedName = name.trim();
    if (!trimmedName) {
      setFormError("El nombre del cliente es obligatorio.");
      return;
    }

    const payload: CreateCustomerInput = {
      name: trimmedName,
      doc_type: docType === "NONE" ? null : docType,
      doc_number: docType === "NONE" || !docNumber.trim() ? null : docNumber.trim(),
      dv: docType === "NIT" && dv.trim() ? dv.trim() : null,
      phone: phone.trim() ? phone.trim() : null,
      email: email.trim() ? email.trim() : null,
    };

    setSaving(true);
    try {
      const res = await api.customers.upsert(payload);
      if (res.result === "UNCHANGED") {
        onSelectCustomer(res.customer);
        onOpenChange(false);
      } else {
        const msg = res.result === "CREATED" ? "Cliente creado" : "Cliente actualizado con los datos nuevos";
        setSuccessMsg(msg);
        setTimeout(() => {
          onSelectCustomer(res.customer);
          onOpenChange(false);
          setSuccessMsg(null);
        }, 500);
      }
    } catch (err) {
      if (err instanceof ApiError && err.code === "CUSTOMER_DUPLICATED") {
        const dupId = (err.data as { id?: string })?.id;
        setDuplicateId(dupId ?? null);
        setFormError("Ya existe un cliente con ese documento.");
      } else {
        setFormError(friendlyError(err));
      }
    } finally {
      setSaving(false);
    }
  }

  async function handleUseDuplicate() {
    if (!duplicateId) return;
    try {
      const list = await api.customers.search({ q: docNumber.trim() });
      const found = list.find((c) => c.id === duplicateId) ?? list[0];
      if (found) {
        onSelectCustomer(found);
        onOpenChange(false);
      }
    } catch (err) {
      setFormError(friendlyError(err));
    }
  }

  function handleSaveAlias(e: React.FormEvent) {
    e.preventDefault();
    const trimmed = alias.trim();
    if (!trimmed || trimmed.length > 40) {
      setAliasError("El alias debe tener entre 1 y 40 caracteres.");
      return;
    }
    onSelectAlias(trimmed);
    onOpenChange(false);
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-md gap-0 rounded-3xl p-0 overflow-hidden sm:max-w-lg">
        <div className="border-b border-slate-100 p-6 pb-4">
          <div className="flex items-center gap-3">
            <span className="flex size-10 items-center justify-center rounded-2xl bg-teal-50 text-brand">
              <User className="size-5" />
            </span>
            <div>
              <DialogTitle className="text-xl font-bold tracking-tight">
                Asignar cliente o mesa
              </DialogTitle>
              <DialogDescription className="text-xs text-slate-500">
                Identificá la venta con un cliente registrado o un alias temporal.
              </DialogDescription>
            </div>
          </div>

          {/* Selector de pestañas */}
          <div className="mt-4 flex rounded-xl bg-slate-100 p-1 text-xs font-semibold">
            <button
              type="button"
              onClick={() => setTab("search")}
              className={cn(
                "flex flex-1 items-center justify-center gap-2 rounded-lg py-2 transition-all",
                tab === "search" ? "bg-white text-brand shadow-xs" : "text-slate-500 hover:text-slate-800",
              )}
            >
              <Search className="size-3.5" />
              Buscar
            </button>
            <button
              type="button"
              onClick={() => setTab("new")}
              className={cn(
                "flex flex-1 items-center justify-center gap-2 rounded-lg py-2 transition-all",
                tab === "new" ? "bg-white text-brand shadow-xs" : "text-slate-500 hover:text-slate-800",
              )}
            >
              <Plus className="size-3.5" />
              Nuevo
            </button>
            {allowAliasSales && (
              <button
                type="button"
                onClick={() => setTab("alias")}
                className={cn(
                  "flex flex-1 items-center justify-center gap-2 rounded-lg py-2 transition-all",
                  tab === "alias" ? "bg-white text-brand shadow-xs" : "text-slate-500 hover:text-slate-800",
                )}
              >
                <Tag className="size-3.5" />
                Alias
              </button>
            )}
          </div>
        </div>

        {/* Tab 1: Buscar */}
        {tab === "search" && (
          <div className="flex flex-col p-6 pt-4">
            <div className="relative mb-4">
              <Search className="pointer-events-none absolute left-3.5 top-1/2 size-4 -translate-y-1/2 text-slate-400" />
              <input
                type="text"
                autoFocus
                value={searchQuery}
                onChange={(e) => setSearchQuery(e.target.value)}
                placeholder="Buscar por nombre, cédula, NIT o teléfono..."
                className="w-full rounded-2xl border border-slate-200 bg-slate-50/50 py-2.5 pl-10 pr-4 text-sm focus:border-brand focus:bg-white focus:outline-none focus:ring-2 focus:ring-brand/20"
              />
              {searching && (
                <Loader2 className="absolute right-3.5 top-1/2 size-4 -translate-y-1/2 animate-spin text-slate-400" />
              )}
            </div>

            <div className="max-h-64 min-h-40 overflow-y-auto divide-y divide-slate-100 rounded-2xl border border-slate-100">
              {searchResults.length === 0 ? (
                <div className="flex h-36 flex-col items-center justify-center gap-2 text-center text-slate-400">
                  <Users className="size-8 stroke-1 text-slate-300" />
                  <p className="text-xs">
                    {searchQuery
                      ? "No se encontraron clientes con ese criterio"
                      : "Escribí para buscar o elegí de la lista"}
                  </p>
                </div>
              ) : (
                searchResults.map((cust) => {
                  const isSelected = currentCustomer?.id === cust.id;
                  return (
                    <button
                      key={cust.id}
                      type="button"
                      onClick={() => {
                        onSelectCustomer(cust);
                        onOpenChange(false);
                      }}
                      className={cn(
                        "flex w-full items-center justify-between p-3 text-left transition-colors hover:bg-slate-50",
                        isSelected && "bg-teal-50/60",
                      )}
                    >
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-2">
                          <p className="truncate text-sm font-semibold text-slate-900">
                            {cust.name}
                          </p>
                          {cust.price_tier_id && (
                            <span className="rounded bg-teal-100 px-1.5 py-0.5 text-[10px] font-bold text-teal-800">
                              Tarifa
                            </span>
                          )}
                        </div>
                        <p className="text-xs text-slate-500">
                          {cust.doc_type && cust.doc_number
                            ? `${cust.doc_type} ${cust.doc_number}${cust.dv ? `-${cust.dv}` : ""}`
                            : "Sin documento"}
                          {cust.phone ? ` · ${cust.phone}` : ""}
                        </p>
                      </div>
                      {isSelected && (
                        <Check className="size-4 shrink-0 text-brand" />
                      )}
                    </button>
                  );
                })
              )}
            </div>

            {currentCustomer && (
              <div className="mt-4 flex justify-between items-center text-xs text-slate-500">
                <span>Cliente actual: <strong className="text-slate-700">{currentCustomer.name}</strong></span>
                <button
                  type="button"
                  onClick={() => {
                    onSelectCustomer(null);
                    onOpenChange(false);
                  }}
                  className="font-medium text-rose-600 hover:underline"
                >
                  Quitar cliente
                </button>
              </div>
            )}
          </div>
        )}

        {/* Tab 2: Nuevo */}
        {tab === "new" && (
          <form onSubmit={handleCreate} className="flex flex-col gap-4 p-6 pt-4">
            {successMsg && (
              <div className="rounded-xl bg-emerald-50 p-3 text-xs text-emerald-800 font-medium flex items-center gap-2">
                <Check className="size-4 text-emerald-600" />
                <span>{successMsg}</span>
              </div>
            )}
            {formError && (
              <div className="rounded-xl bg-rose-50 p-3 text-xs text-rose-700 flex flex-col gap-2">
                <span>{formError}</span>
                {duplicateId && (
                  <button
                    type="button"
                    onClick={handleUseDuplicate}
                    className="self-start rounded-lg bg-rose-700 px-3 py-1 font-semibold text-white text-xs hover:bg-rose-800"
                  >
                    Usar el existente
                  </button>
                )}
              </div>
            )}

            <div>
              <label className="mb-1 block text-xs font-semibold text-slate-700">
                Nombre o Razón Social *
              </label>
              <input
                type="text"
                required
                maxLength={120}
                value={name}
                onChange={(e) => setName(e.target.value)}
                placeholder="Ej. Comercializadora Andina SAS / Juan Pérez"
                className="w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm focus:border-brand focus:outline-none focus:ring-2 focus:ring-brand/20"
              />
            </div>

            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="mb-1 block text-xs font-semibold text-slate-700">
                  Tipo de documento
                </label>
                <select
                  value={docType}
                  onChange={(e) => setDocType(e.target.value as DocType | "NONE")}
                  className="w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm focus:border-brand focus:outline-none focus:ring-2 focus:ring-brand/20"
                >
                  {DOC_TYPES.map((dt) => (
                    <option key={dt.code} value={dt.code}>
                      {dt.label}
                    </option>
                  ))}
                </select>
              </div>

              {docType !== "NONE" && (
                <div className="flex gap-2">
                  <div className="flex-1">
                    <label className="mb-1 block text-xs font-semibold text-slate-700">
                      Número
                    </label>
                    <input
                      type="text"
                      maxLength={20}
                      value={docNumber}
                      onChange={(e) => setDocNumber(e.target.value)}
                      placeholder="123456789"
                      className="w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm uppercase focus:border-brand focus:outline-none focus:ring-2 focus:ring-brand/20"
                    />
                  </div>
                  {docType === "NIT" && (
                    <div className="w-14">
                      <label className="mb-1 block text-xs font-semibold text-slate-700">
                        DV
                      </label>
                      <input
                        type="text"
                        maxLength={1}
                        value={dv}
                        onChange={(e) => setDv(e.target.value)}
                        placeholder="0"
                        className="w-full text-center rounded-xl border border-slate-200 bg-white px-2 py-2 text-sm focus:border-brand focus:outline-none focus:ring-2 focus:ring-brand/20"
                      />
                    </div>
                  )}
                </div>
              )}
            </div>

            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="mb-1 block text-xs font-semibold text-slate-700">
                  Teléfono
                </label>
                <input
                  type="tel"
                  maxLength={15}
                  value={phone}
                  onChange={(e) => setPhone(e.target.value)}
                  placeholder="3001234567"
                  className="w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm focus:border-brand focus:outline-none focus:ring-2 focus:ring-brand/20"
                />
              </div>
              <div>
                <label className="mb-1 block text-xs font-semibold text-slate-700">
                  Correo electrónico
                </label>
                <input
                  type="email"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  placeholder="cliente@correo.com"
                  className="w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm lowercase focus:border-brand focus:outline-none focus:ring-2 focus:ring-brand/20"
                />
              </div>
            </div>

            <div className="mt-2 flex justify-end gap-2">
              <button
                type="button"
                onClick={() => onOpenChange(false)}
                className="rounded-xl border border-slate-200 px-4 py-2 text-xs font-semibold text-slate-600 hover:bg-slate-50"
              >
                Cancelar
              </button>
              <button
                type="submit"
                disabled={saving}
                className="flex items-center gap-2 rounded-xl bg-brand px-5 py-2 text-xs font-bold text-white shadow-md shadow-teal-900/10 hover:bg-brand-dark disabled:opacity-50"
              >
                {saving && <Loader2 className="size-3.5 animate-spin" />}
                Guardar y asignar
              </button>
            </div>
          </form>
        )}

        {/* Tab 3: Alias */}
        {tab === "alias" && allowAliasSales && (
          <form onSubmit={handleSaveAlias} className="flex flex-col gap-4 p-6 pt-4">
            {aliasError && (
              <div className="rounded-xl bg-rose-50 p-3 text-xs text-rose-700">
                {aliasError}
              </div>
            )}

            <div>
              <label className="mb-1 block text-xs font-semibold text-slate-700">
                Nombre de mesa o alias identificador
              </label>
              <input
                type="text"
                autoFocus
                maxLength={40}
                value={alias}
                onChange={(e) => setAlias(e.target.value)}
                placeholder="Ej. Mesa 4, Domicilio Carlos, Mostrador 2..."
                className="w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm focus:border-brand focus:outline-none focus:ring-2 focus:ring-brand/20"
              />
              <p className="mt-1 text-[11px] text-slate-400">
                Usá un alias cuando la venta no tenga un cliente con cédula o NIT registrado.
              </p>
            </div>

            <div className="mt-2 flex justify-between items-center">
              {currentAlias ? (
                <button
                  type="button"
                  onClick={() => {
                    onSelectAlias(null);
                    onOpenChange(false);
                  }}
                  className="text-xs font-medium text-rose-600 hover:underline"
                >
                  Quitar alias
                </button>
              ) : <div />}

              <div className="flex gap-2">
                <button
                  type="button"
                  onClick={() => onOpenChange(false)}
                  className="rounded-xl border border-slate-200 px-4 py-2 text-xs font-semibold text-slate-600 hover:bg-slate-50"
                >
                  Cancelar
                </button>
                <button
                  type="submit"
                  className="rounded-xl bg-brand px-5 py-2 text-xs font-bold text-white shadow-md shadow-teal-900/10 hover:bg-brand-dark"
                >
                  Usar alias
                </button>
              </div>
            </div>
          </form>
        )}
      </DialogContent>
    </Dialog>
  );
}
