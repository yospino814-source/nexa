"use client";

import { useState } from "react";
import { Building2, Loader2 } from "lucide-react";

import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { api, friendlyError, type Supplier } from "@/lib/api";

export function SupplierDialog({
  supplier,
  open,
  onClose,
  onSaved,
}: {
  supplier?: Supplier | null;
  open: boolean;
  onClose: () => void;
  onSaved: (supplier: Supplier) => void;
}) {
  const isEditing = !!supplier;
  const [name, setName] = useState("");
  const [docType, setDocType] = useState<"NIT" | "CC" | "CE">("NIT");
  const [taxId, setTaxId] = useState("");
  const [dv, setDv] = useState("");
  const [personType, setPersonType] = useState<"NATURAL" | "JURIDICA">("JURIDICA");
  const [ivaResponsible, setIvaResponsible] = useState(true);
  const [selfWithholding, setSelfWithholding] = useState(false);
  const [paymentTerms, setPaymentTerms] = useState<"CONTADO" | "CREDITO">("CONTADO");
  const [creditDays, setCreditDays] = useState(0);
  const [deliveryDays, setDeliveryDays] = useState<number | "">("");
  const [category, setCategory] = useState<"MERCANCIA" | "TRANSPORTE" | "SERVICIOS">("MERCANCIA");
  const [bankName, setBankName] = useState("");
  const [bankAccountType, setBankAccountType] = useState<"AHORROS" | "CORRIENTE" | "">("");
  const [bankAccountNumber, setBankAccountNumber] = useState("");
  const [contactName, setContactName] = useState("");
  const [phone, setPhone] = useState("");
  const [email, setEmail] = useState("");
  const [address, setAddress] = useState("");
  const [notes, setNotes] = useState("");
  const [active, setActive] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const openKey = open ? (supplier ? `edit:${supplier.id}` : "new") : null;
  const [lastKey, setLastKey] = useState(openKey);
  if (openKey !== lastKey) {
    setLastKey(openKey);
    if (supplier) {
      setName(supplier.name);
      setDocType(supplier.doc_type ?? "NIT");
      setTaxId(supplier.tax_id ?? "");
      setDv(supplier.dv ?? "");
      setPersonType(supplier.person_type ?? "JURIDICA");
      setIvaResponsible(supplier.iva_responsible ?? true);
      setSelfWithholding(supplier.self_withholding ?? false);
      setPaymentTerms(supplier.payment_terms ?? "CONTADO");
      setCreditDays(supplier.credit_days ?? 0);
      setDeliveryDays(supplier.delivery_days ?? "");
      setCategory(supplier.category ?? "MERCANCIA");
      setBankName(supplier.bank_name ?? "");
      setBankAccountType(supplier.bank_account_type ?? "");
      setBankAccountNumber(supplier.bank_account_number ?? "");
      setContactName(supplier.contact_name ?? "");
      setPhone(supplier.phone ?? "");
      setEmail(supplier.email ?? "");
      setAddress(supplier.address ?? "");
      setNotes(supplier.notes ?? "");
      setActive(supplier.active);
    } else {
      setName("");
      setDocType("NIT");
      setTaxId("");
      setDv("");
      setPersonType("JURIDICA");
      setIvaResponsible(true);
      setSelfWithholding(false);
      setPaymentTerms("CONTADO");
      setCreditDays(0);
      setDeliveryDays("");
      setCategory("MERCANCIA");
      setBankName("");
      setBankAccountType("");
      setBankAccountNumber("");
      setContactName("");
      setPhone("");
      setEmail("");
      setAddress("");
      setNotes("");
      setActive(true);
    }
    setError(null);
  }

  if (!open) return null;

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    if (!name.trim()) {
      setError("Escribí el nombre del proveedor.");
      return;
    }

    setBusy(true);
    setError(null);

    const payload = {
      name: name.trim(),
      doc_type: docType,
      tax_id: taxId.trim() || null,
      dv: docType === "NIT" ? (dv.trim() || null) : null,
      person_type: personType,
      iva_responsible: ivaResponsible,
      self_withholding: selfWithholding,
      payment_terms: paymentTerms,
      credit_days: paymentTerms === "CREDITO" ? creditDays : 0,
      delivery_days: deliveryDays === "" ? null : Number(deliveryDays),
      category,
      bank_name: bankName.trim() || null,
      bank_account_type: bankAccountType || null,
      bank_account_number: bankAccountNumber.trim() || null,
      contact_name: contactName.trim() || null,
      phone: phone.trim() || null,
      email: email.trim() || null,
      address: address.trim() || null,
      notes: notes.trim() || null,
    };

    try {
      if (isEditing && supplier) {
        const updated = await api.suppliers.update(supplier.id, {
          ...payload,
          active,
        });
        onSaved(updated);
      } else {
        const created = await api.suppliers.create(payload);
        onSaved(created);
      }
      onClose();
    } catch (err) {
      setError(friendlyError(err));
    } finally {
      setBusy(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={(v) => !v && onClose()}>
      <DialogContent className="max-h-[92vh] overflow-y-auto rounded-3xl p-6 sm:max-w-lg">
        <div className="flex items-start gap-3">
          <span className="flex size-11 shrink-0 items-center justify-center rounded-2xl bg-teal-50 text-teal-700">
            <Building2 className="size-5" />
          </span>
          <div>
            <DialogTitle className="text-lg font-bold">
              {isEditing ? "Editar proveedor" : "Nuevo proveedor"}
            </DialogTitle>
            <DialogDescription className="text-sm text-slate-500">
              {isEditing
                ? "Modificá los datos del proveedor para todo el negocio."
                : "Registrá un nuevo distribuidor o proveedor comercial."}
            </DialogDescription>
          </div>
        </div>

        {error && (
          <div className="mt-4 rounded-xl border border-rose-200 bg-rose-50 p-3 text-sm text-rose-700">
            {error}
          </div>
        )}

        <form onSubmit={handleSubmit} className="mt-4 space-y-4">
          <div>
            <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
              Nombre o razón social *
            </label>
            <input
              required
              type="text"
              placeholder="Ej. Distribuidora Los Andes S.A.S."
              value={name}
              onChange={(e) => setName(e.target.value)}
              className="mt-1 w-full rounded-xl border border-slate-200 px-3.5 py-2 text-sm text-slate-800 placeholder-slate-400 focus:border-teal-700 focus:outline-none focus:ring-2 focus:ring-teal-700/20"
            />
          </div>

          <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
            <div>
              <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                Tipo Doc
              </label>
              <select
                value={docType}
                onChange={(e) => setDocType(e.target.value as "NIT" | "CC" | "CE")}
                className="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-800 focus:border-teal-700 focus:outline-none focus:ring-2 focus:ring-teal-700/20"
              >
                <option value="NIT">NIT</option>
                <option value="CC">Cédula (CC)</option>
                <option value="CE">Cédula Ext. (CE)</option>
              </select>
            </div>
            <div>
              <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                Número
              </label>
              <input
                type="text"
                placeholder="Ej. 900123456"
                value={taxId}
                onChange={(e) => setTaxId(e.target.value)}
                className="mt-1 w-full rounded-xl border border-slate-200 px-3.5 py-2 text-sm text-slate-800 placeholder-slate-400 focus:border-teal-700 focus:outline-none focus:ring-2 focus:ring-teal-700/20"
              />
            </div>
            {docType === "NIT" && (
              <div>
                <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                  DV (opcional)
                </label>
                <input
                  type="text"
                  maxLength={1}
                  placeholder="Ej. 8"
                  value={dv}
                  onChange={(e) => setDv(e.target.value)}
                  className="mt-1 w-full rounded-xl border border-slate-200 px-3.5 py-2 text-sm text-slate-800 placeholder-slate-400 focus:border-teal-700 focus:outline-none focus:ring-2 focus:ring-teal-700/20"
                />
              </div>
            )}
          </div>

          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <div>
              <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                Tipo de persona
              </label>
              <select
                value={personType}
                onChange={(e) => setPersonType(e.target.value as "NATURAL" | "JURIDICA")}
                className="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-800 focus:border-teal-700 focus:outline-none focus:ring-2 focus:ring-teal-700/20"
              >
                <option value="JURIDICA">Persona Jurídica</option>
                <option value="NATURAL">Persona Natural</option>
              </select>
            </div>
            <div>
              <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                Categoría
              </label>
              <select
                value={category}
                onChange={(e) => setCategory(e.target.value as "MERCANCIA" | "TRANSPORTE" | "SERVICIOS")}
                className="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-800 focus:border-teal-700 focus:outline-none focus:ring-2 focus:ring-teal-700/20"
              >
                <option value="MERCANCIA">Mercancía</option>
                <option value="TRANSPORTE">Transporte</option>
                <option value="SERVICIOS">Servicios</option>
              </select>
            </div>
          </div>

          <div className="flex flex-wrap items-center gap-6 py-1">
            <label className="flex items-center gap-2 text-sm text-slate-700 cursor-pointer">
              <input
                type="checkbox"
                checked={ivaResponsible}
                onChange={(e) => setIvaResponsible(e.target.checked)}
                className="size-4 rounded border-slate-300 text-teal-700 focus:ring-teal-700"
              />
              Responsable de IVA
            </label>
            <label className="flex items-center gap-2 text-sm text-slate-700 cursor-pointer">
              <input
                type="checkbox"
                checked={selfWithholding}
                onChange={(e) => setSelfWithholding(e.target.checked)}
                className="size-4 rounded border-slate-300 text-teal-700 focus:ring-teal-700"
              />
              Autorretenedor
            </label>
          </div>

          <div className="grid grid-cols-1 gap-4 sm:grid-cols-3">
            <div>
              <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                Condiciones de pago
              </label>
              <select
                value={paymentTerms}
                onChange={(e) => setPaymentTerms(e.target.value as "CONTADO" | "CREDITO")}
                className="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-800 focus:border-teal-700 focus:outline-none focus:ring-2 focus:ring-teal-700/20"
              >
                <option value="CONTADO">Contado</option>
                <option value="CREDITO">Crédito</option>
              </select>
            </div>
            {paymentTerms === "CREDITO" && (
              <div>
                <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                  Días de crédito
                </label>
                <input
                  type="number"
                  min={0}
                  max={180}
                  value={creditDays}
                  onChange={(e) => setCreditDays(Number(e.target.value))}
                  className="mt-1 w-full rounded-xl border border-slate-200 px-3.5 py-2 text-sm text-slate-800 focus:border-teal-700 focus:outline-none focus:ring-2 focus:ring-teal-700/20"
                />
              </div>
            )}
            <div>
              <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                Días de entrega
              </label>
              <input
                type="number"
                min={0}
                max={60}
                placeholder="Ej. 3"
                value={deliveryDays}
                onChange={(e) => setDeliveryDays(e.target.value === "" ? "" : Number(e.target.value))}
                className="mt-1 w-full rounded-xl border border-slate-200 px-3.5 py-2 text-sm text-slate-800 placeholder-slate-400 focus:border-teal-700 focus:outline-none focus:ring-2 focus:ring-teal-700/20"
              />
            </div>
          </div>

          <div className="rounded-2xl border border-slate-100 bg-slate-50/50 p-4 space-y-3">
            <p className="text-xs font-semibold uppercase tracking-wider text-slate-500">
              Datos Bancarios
            </p>
            <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
              <div>
                <label className="text-xs text-slate-500">Banco</label>
                <input
                  type="text"
                  placeholder="Ej. Bancolombia"
                  value={bankName}
                  onChange={(e) => setBankName(e.target.value)}
                  className="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-1.5 text-sm text-slate-800 placeholder-slate-400 focus:border-teal-700 focus:outline-none focus:ring-2 focus:ring-teal-700/20"
                />
              </div>
              <div>
                <label className="text-xs text-slate-500">Tipo de cuenta</label>
                <select
                  value={bankAccountType}
                  onChange={(e) => setBankAccountType(e.target.value as "AHORROS" | "CORRIENTE" | "")}
                  className="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-1.5 text-sm text-slate-800 focus:border-teal-700 focus:outline-none focus:ring-2 focus:ring-teal-700/20"
                >
                  <option value="">Seleccionar...</option>
                  <option value="AHORROS">Ahorros</option>
                  <option value="CORRIENTE">Corriente</option>
                </select>
              </div>
              <div>
                <label className="text-xs text-slate-500">Número de cuenta</label>
                <input
                  type="text"
                  placeholder="6 a 20 dígitos"
                  value={bankAccountNumber}
                  onChange={(e) => setBankAccountNumber(e.target.value)}
                  className="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-1.5 text-sm text-slate-800 placeholder-slate-400 focus:border-teal-700 focus:outline-none focus:ring-2 focus:ring-teal-700/20"
                />
              </div>
            </div>
          </div>

          <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
            <div>
              <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                Teléfono / WhatsApp
              </label>
              <input
                type="text"
                placeholder="Ej. 310 123 4567"
                value={phone}
                onChange={(e) => setPhone(e.target.value)}
                className="mt-1 w-full rounded-xl border border-slate-200 px-3.5 py-2 text-sm text-slate-800 placeholder-slate-400 focus:border-teal-700 focus:outline-none focus:ring-2 focus:ring-teal-700/20"
              />
            </div>
            <div>
              <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
                Correo electrónico
              </label>
              <input
                type="email"
                placeholder="pedidos@empresa.com"
                value={email}
                onChange={(e) => setEmail(e.target.value)}
                className="mt-1 w-full rounded-xl border border-slate-200 px-3.5 py-2 text-sm text-slate-800 placeholder-slate-400 focus:border-teal-700 focus:outline-none focus:ring-2 focus:ring-teal-700/20"
              />
            </div>
          </div>

          <div>
            <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
              Dirección
            </label>
            <input
              type="text"
              placeholder="Ej. Av. Boyacá # 72-10, Bogotá"
              value={address}
              onChange={(e) => setAddress(e.target.value)}
              className="mt-1 w-full rounded-xl border border-slate-200 px-3.5 py-2 text-sm text-slate-800 placeholder-slate-400 focus:border-teal-700 focus:outline-none focus:ring-2 focus:ring-teal-700/20"
            />
          </div>

          <div>
            <label className="text-xs font-semibold uppercase tracking-wider text-slate-500">
              Notas u observaciones
            </label>
            <textarea
              rows={2}
              placeholder="Condiciones de despacho, días de entrega, etc."
              value={notes}
              onChange={(e) => setNotes(e.target.value)}
              className="mt-1 w-full rounded-xl border border-slate-200 px-3.5 py-2 text-sm text-slate-800 placeholder-slate-400 focus:border-teal-700 focus:outline-none focus:ring-2 focus:ring-teal-700/20"
            />
          </div>

          {isEditing && (
            <div className="flex items-center gap-3 pt-1">
              <input
                type="checkbox"
                id="active"
                checked={active}
                onChange={(e) => setActive(e.target.checked)}
                className="size-4 rounded border-slate-300 text-teal-700 focus:ring-teal-700"
              />
              <label htmlFor="active" className="text-sm font-medium text-slate-700">
                Proveedor activo (aparece al crear compras)
              </label>
            </div>
          )}

          <div className="mt-6 flex justify-end gap-3 pt-2">
            <button
              type="button"
              onClick={onClose}
              disabled={busy}
              className="h-11 rounded-full border border-slate-200 px-5 text-sm font-medium text-slate-700 hover:bg-slate-50 disabled:opacity-50"
            >
              Cancelar
            </button>
            <button
              type="submit"
              disabled={busy}
              className="flex h-11 items-center gap-2 rounded-full bg-teal-700 px-6 text-sm font-semibold text-white shadow-sm hover:bg-teal-800 disabled:opacity-50"
            >
              {busy && <Loader2 className="size-4 animate-spin" />}
              {isEditing ? "Guardar cambios" : "Crear proveedor"}
            </button>
          </div>
        </form>
      </DialogContent>
    </Dialog>
  );
}
