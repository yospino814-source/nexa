"use client";

import { useState } from "react";
import { AlertCircle, Check, Loader2, User } from "lucide-react";

import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  api,
  ApiError,
  friendlyError,
  type CreateCustomerInput,
  type Customer,
  type CustomerDetail,
  type DocType,
} from "@/lib/api";
import { defaultPersonType, type PersonType } from "@/lib/customers";
import { useSession } from "@/lib/stores/session";

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

const FISCAL_RESPONSIBILITIES = [
  { code: "O-13", label: "O-13 Gran contribuyente" },
  { code: "O-15", label: "O-15 Autorretenedor" },
  { code: "O-23", label: "O-23 Agente de retención IVA" },
  { code: "O-47", label: "O-47 Régimen simple" },
  { code: "R-99-PN", label: "R-99-PN No responsable" },
];

export function CustomerFormDialog({
  open,
  onOpenChange,
  customer,
  onSuccess,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  customer?: Customer | CustomerDetail | null;
  onSuccess: (saved: CustomerDetail) => void;
}) {
  const canOverrideCredit = useSession((s) => s.can("credit:override"));

  const [name, setName] = useState("");
  const [docType, setDocType] = useState<DocType | "NONE">("CC");
  const [docNumber, setDocNumber] = useState("");
  const [dv, setDv] = useState("");
  const [personType, setPersonType] = useState<PersonType>("NATURAL");
  const [personTypeTouched, setPersonTypeTouched] = useState(false);
  const [phone, setPhone] = useState("");
  const [email, setEmail] = useState("");
  const [taxRegime, setTaxRegime] = useState<"NO_RESPONSABLE" | "RESPONSABLE_IVA">("NO_RESPONSABLE");
  const [responsibilities, setResponsibilities] = useState<string[]>(["R-99-PN"]);
  const [ciiu, setCiiu] = useState("");
  const [withholdingAgent, setWithholdingAgent] = useState(false);
  const [cityCode, setCityCode] = useState("");
  const [billingAddress, setBillingAddress] = useState("");
  const [shippingAddress, setShippingAddress] = useState("");
  const [pinnedNote, setPinnedNote] = useState("");
  const [creditLimit, setCreditLimit] = useState("0");
  const [creditBlockDays, setCreditBlockDays] = useState("30");

  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  // Cargar datos si estamos en modo edición
  const [lastOpen, setLastOpen] = useState(open);
  if (open !== lastOpen) {
    setLastOpen(open);
    if (open) {
      setError(null);
      if (customer) {
        setName(customer.name || "");
        const dt = (customer.doc_type as DocType) || "NONE";
        setDocType(dt);
        const dn = customer.doc_number || "";
        setDocNumber(dn);
        setDv(customer.dv || "");
        setPersonType(customer.person_type === "JURIDICA" ? "JURIDICA" : "NATURAL");
        setPersonTypeTouched(Boolean(customer.person_type));
        setPhone(customer.phone || "");
        setEmail(customer.email || "");
        setTaxRegime(customer.tax_regime === "RESPONSABLE_IVA" ? "RESPONSABLE_IVA" : "NO_RESPONSABLE");
        setResponsibilities(customer.fiscal_responsibilities?.length ? customer.fiscal_responsibilities : ["R-99-PN"]);
        setCiiu(customer.ciiu || "");
        setWithholdingAgent(Boolean(customer.withholding_agent));
        setCityCode(customer.city_code || "");
        setBillingAddress(customer.billing_address || "");
        setShippingAddress(customer.shipping_address || "");
        setPinnedNote(customer.pinned_note || "");
        setCreditLimit(String(customer.credit_limit ?? 0));
        setCreditBlockDays(String(customer.credit_block_days ?? 30));
      } else {
        setName("");
        setDocType("CC");
        setDocNumber("");
        setDv("");
        setPersonType("NATURAL");
        setPersonTypeTouched(false);
        setPhone("");
        setEmail("");
        setTaxRegime("NO_RESPONSABLE");
        setResponsibilities(["R-99-PN"]);
        setCiiu("");
        setWithholdingAgent(false);
        setCityCode("");
        setBillingAddress("");
        setShippingAddress("");
        setPinnedNote("");
        setCreditLimit("0");
        setCreditBlockDays("30");
      }
    }
  }

  const toggleResp = (code: string) => {
    if (responsibilities.includes(code)) {
      setResponsibilities(responsibilities.filter((c) => c !== code));
    } else {
      setResponsibilities([...responsibilities, code]);
    }
  };

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();
    setError(null);

    const trimmedName = name.trim();
    if (!trimmedName) {
      setError("El nombre del cliente es obligatorio.");
      return;
    }

    const nBlockDays = parseInt(creditBlockDays, 10);
    const payload: CreateCustomerInput = {
      name: trimmedName,
      doc_type: docType === "NONE" ? null : docType,
      doc_number: docType === "NONE" || !docNumber.trim() ? null : docNumber.trim(),
      dv: docType === "NIT" && dv.trim() ? dv.trim() : null,
      person_type: docType === "NIT" ? personType : "NATURAL",
      phone: phone.trim() ? phone.trim() : null,
      email: email.trim() ? email.trim() : null,
      tax_regime: taxRegime,
      fiscal_responsibilities: responsibilities.length > 0 ? responsibilities : ["R-99-PN"],
      ciiu: ciiu.trim() ? ciiu.trim() : null,
      withholding_agent: withholdingAgent,
      city_code: cityCode.trim() ? cityCode.trim() : null,
      billing_address: billingAddress.trim() ? billingAddress.trim() : null,
      shipping_address: shippingAddress.trim() ? shippingAddress.trim() : null,
      pinned_note: pinnedNote.trim() ? pinnedNote.trim() : null,
      credit_limit: creditLimit.trim() ? creditLimit.trim() : "0.00",
      credit_block_days: Number.isNaN(nBlockDays) ? 30 : nBlockDays,
    };

    setSaving(true);
    try {
      let saved: CustomerDetail;
      if (customer?.id) {
        saved = await api.customers.update(customer.id, payload);
      } else {
        saved = await api.customers.create(payload);
      }
      onSuccess(saved);
      onOpenChange(false);
    } catch (err) {
      if (err instanceof ApiError && err.code === "CUSTOMER_DUPLICATED") {
        setError("Ya existe un cliente registrado con ese documento.");
      } else {
        setError(friendlyError(err));
      }
    } finally {
      setSaving(false);
    }
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl max-h-[90vh] overflow-y-auto rounded-3xl p-6">
        <DialogHeader>
          <div className="flex items-center gap-3 mb-1">
            <span className="flex size-10 items-center justify-center rounded-2xl bg-teal-50 text-teal-700">
              <User className="size-5" />
            </span>
            <div>
              <DialogTitle className="text-xl font-bold tracking-tight text-slate-900">
                {customer ? "Editar ficha de cliente" : "Nuevo cliente"}
              </DialogTitle>
              <DialogDescription className="text-xs text-slate-500">
                Información fiscal, datos de contacto y condiciones comerciales.
              </DialogDescription>
            </div>
          </div>
        </DialogHeader>

        {error && (
          <div className="mt-3 flex items-center gap-2 rounded-xl bg-rose-50 p-3 text-xs text-rose-700">
            <AlertCircle className="size-4 shrink-0" />
            <span>{error}</span>
          </div>
        )}

        <form onSubmit={handleSubmit} className="mt-4 flex flex-col gap-5">
          {/* Identificación y Nombre */}
          <div className={`grid grid-cols-1 ${docType === "NIT" ? "md:grid-cols-4" : "md:grid-cols-3"} gap-3`}>
            <div className="md:col-span-1">
              <label className="mb-1 block text-xs font-semibold text-slate-700">
                Tipo de documento
              </label>
              <select
                value={docType}
                onChange={(e) => {
                  const newType = e.target.value as DocType | "NONE";
                  setDocType(newType);
                  if (!personTypeTouched) {
                    setPersonType(defaultPersonType(newType, docNumber));
                  }
                }}
                className="w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-xs font-medium text-slate-800 focus:border-teal-600 focus:outline-hidden"
              >
                {DOC_TYPES.map((dt) => (
                  <option key={dt.code} value={dt.code}>
                    {dt.label}
                  </option>
                ))}
              </select>
            </div>

            <div className={docType === "NIT" ? "md:col-span-1" : "md:col-span-2"}>
              <label className="mb-1 block text-xs font-semibold text-slate-700">
                Número de documento
              </label>
              <input
                type="text"
                disabled={docType === "NONE"}
                value={docNumber}
                onChange={(e) => {
                  const val = e.target.value;
                  setDocNumber(val);
                  if (!personTypeTouched && docType === "NIT") {
                    setPersonType(defaultPersonType("NIT", val));
                  }
                }}
                placeholder={docType === "NONE" ? "No aplica" : "Ej: 900123456"}
                className="w-full rounded-xl border border-slate-200 px-3 py-2 text-xs font-medium text-slate-800 placeholder:text-slate-400 focus:border-teal-600 focus:outline-hidden disabled:bg-slate-50"
              />
            </div>

            {docType === "NIT" && (
              <>
                <div className="md:col-span-1">
                  <label className="mb-1 block text-xs font-semibold text-slate-700">
                    Dígito (DV)
                  </label>
                  <input
                    type="text"
                    maxLength={1}
                    value={dv}
                    onChange={(e) => setDv(e.target.value)}
                    placeholder="Auto"
                    className="w-full rounded-xl border border-slate-200 px-3 py-2 text-xs font-medium text-slate-800 text-center placeholder:text-slate-400 focus:border-teal-600 focus:outline-hidden"
                  />
                </div>

                <div className="md:col-span-1">
                  <label className="mb-1 block text-xs font-semibold text-slate-700">
                    Tipo de persona
                  </label>
                  <select
                    value={personType}
                    onChange={(e) => {
                      setPersonType(e.target.value as PersonType);
                      setPersonTypeTouched(true);
                    }}
                    className="w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-xs font-medium text-slate-800 focus:border-teal-600 focus:outline-hidden"
                  >
                    <option value="NATURAL">Persona Natural</option>
                    <option value="JURIDICA">Persona Jurídica</option>
                  </select>
                </div>
              </>
            )}
          </div>

          <div>
            <label className="mb-1 block text-xs font-semibold text-slate-700">
              {docType === "NIT" && personType === "JURIDICA" ? "Razón social" : "Nombre completo"} <span className="text-rose-500">*</span>
            </label>
            <input
              type="text"
              required
              value={name}
              onChange={(e) => setName(e.target.value)}
              placeholder={docType === "NIT" && personType === "JURIDICA" ? "Ej: Distribuidora El Triunfo SAS" : "Ej: Juan Pérez"}
              className="w-full rounded-xl border border-slate-200 px-3 py-2 text-xs font-medium text-slate-800 placeholder:text-slate-400 focus:border-teal-600 focus:outline-hidden"
            />
          </div>

          {/* Contacto */}
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            <div>
              <label className="mb-1 block text-xs font-semibold text-slate-700">
                Teléfono / WhatsApp
              </label>
              <input
                type="tel"
                value={phone}
                onChange={(e) => setPhone(e.target.value)}
                placeholder="Ej: 3001234567"
                className="w-full rounded-xl border border-slate-200 px-3 py-2 text-xs font-medium text-slate-800 placeholder:text-slate-400 focus:border-teal-600 focus:outline-hidden"
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
                placeholder="cliente@ejemplo.com"
                className="w-full rounded-xl border border-slate-200 px-3 py-2 text-xs font-medium text-slate-800 placeholder:text-slate-400 focus:border-teal-600 focus:outline-hidden"
              />
            </div>
          </div>

          {/* Datos Fiscales */}
          <div className="rounded-2xl border border-slate-200 bg-slate-50/50 p-4">
            <h4 className="text-xs font-bold text-slate-800 mb-3 uppercase tracking-wider">
              Datos para Facturación Electrónica
            </h4>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
              <div>
                <label className="mb-1 block text-xs font-semibold text-slate-700">
                  Régimen tributario
                </label>
                <select
                  value={taxRegime}
                  onChange={(e) => setTaxRegime(e.target.value as "NO_RESPONSABLE" | "RESPONSABLE_IVA")}
                  className="w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-xs font-medium text-slate-800 focus:border-teal-600 focus:outline-hidden"
                >
                  <option value="NO_RESPONSABLE">No responsable de IVA</option>
                  <option value="RESPONSABLE_IVA">Responsable de IVA</option>
                </select>
              </div>

              <div>
                <label className="mb-1 block text-xs font-semibold text-slate-700">
                  Código Municipio (DANE)
                </label>
                <input
                  type="text"
                  maxLength={5}
                  value={cityCode}
                  onChange={(e) => setCityCode(e.target.value)}
                  placeholder="Ej: 05001"
                  className="w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-xs font-medium text-slate-800 placeholder:text-slate-400 focus:border-teal-600 focus:outline-hidden"
                />
              </div>

              <div>
                <label className="mb-1 block text-xs font-semibold text-slate-700">
                  Código CIIU
                </label>
                <input
                  type="text"
                  maxLength={4}
                  value={ciiu}
                  onChange={(e) => setCiiu(e.target.value)}
                  placeholder="Ej: 4711"
                  className="w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-xs font-medium text-slate-800 placeholder:text-slate-400 focus:border-teal-600 focus:outline-hidden"
                />
              </div>
            </div>

            <div className="mt-3">
              <label className="mb-1 block text-xs font-semibold text-slate-700">
                Responsabilidades fiscales
              </label>
              <div className="flex flex-wrap gap-2">
                {FISCAL_RESPONSIBILITIES.map((r) => {
                  const active = responsibilities.includes(r.code);
                  return (
                    <button
                      key={r.code}
                      type="button"
                      onClick={() => toggleResp(r.code)}
                      className={`inline-flex items-center gap-1.5 rounded-lg px-2.5 py-1 text-xs font-medium transition-colors ${
                        active
                          ? "bg-teal-700 text-white"
                          : "bg-white border border-slate-200 text-slate-700 hover:bg-slate-100"
                      }`}
                    >
                      {active && <Check className="size-3" />}
                      {r.label}
                    </button>
                  );
                })}
              </div>
            </div>

            <div className="mt-3">
              <label className="flex items-center gap-2 cursor-pointer text-xs font-semibold text-slate-700">
                <input
                  type="checkbox"
                  checked={withholdingAgent}
                  onChange={(e) => setWithholdingAgent(e.target.checked)}
                  className="rounded border-slate-300 text-teal-600 focus:ring-teal-500"
                />
                Agente de retención en la fuente
              </label>
            </div>
          </div>

          {/* Direcciones */}
          <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
            <div>
              <label className="mb-1 block text-xs font-semibold text-slate-700">
                Dirección de facturación
              </label>
              <input
                type="text"
                maxLength={200}
                value={billingAddress}
                onChange={(e) => setBillingAddress(e.target.value)}
                placeholder="Ej: Cra 43A # 1-50 Of 301"
                className="w-full rounded-xl border border-slate-200 px-3 py-2 text-xs font-medium text-slate-800 placeholder:text-slate-400 focus:border-teal-600 focus:outline-hidden"
              />
            </div>

            <div>
              <label className="mb-1 block text-xs font-semibold text-slate-700">
                Dirección de despacho / entrega
              </label>
              <input
                type="text"
                maxLength={200}
                value={shippingAddress}
                onChange={(e) => setShippingAddress(e.target.value)}
                placeholder="Ej: Bodega 4 Parque Industrial"
                className="w-full rounded-xl border border-slate-200 px-3 py-2 text-xs font-medium text-slate-800 placeholder:text-slate-400 focus:border-teal-600 focus:outline-hidden"
              />
            </div>
          </div>

          {/* Condiciones Comerciales y Cupo de Crédito */}
          <div className="rounded-2xl border border-slate-200 bg-slate-50/50 p-4">
            <div className="flex items-center justify-between mb-2">
              <h4 className="text-xs font-bold text-slate-800 uppercase tracking-wider">
                Condiciones de Crédito (Fiado)
              </h4>
            </div>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
              <div>
                <label className="mb-1 block text-xs font-semibold text-slate-700">
                  Cupo de crédito ($ COP)
                </label>
                <input
                  type="number"
                  min={0}
                  step={1000}
                  disabled={!canOverrideCredit}
                  value={creditLimit}
                  onChange={(e) => setCreditLimit(e.target.value)}
                  placeholder="0"
                  className="w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-xs font-medium text-slate-800 placeholder:text-slate-400 focus:border-teal-600 focus:outline-hidden disabled:bg-slate-100"
                />
              </div>

              <div>
                <label className="mb-1 block text-xs font-semibold text-slate-700">
                  Días de tolerancia mora
                </label>
                <input
                  type="number"
                  min={0}
                  max={365}
                  disabled={!canOverrideCredit}
                  value={creditBlockDays}
                  onChange={(e) => setCreditBlockDays(e.target.value)}
                  placeholder="30"
                  className="w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-xs font-medium text-slate-800 placeholder:text-slate-400 focus:border-teal-600 focus:outline-hidden disabled:bg-slate-100"
                />
              </div>
            </div>
            {!canOverrideCredit && (
              <p className="mt-2 text-[11px] text-slate-500">
                Solo un administrador puede cambiar el cupo.
              </p>
            )}
          </div>

          <div>
            <label className="mb-1 block text-xs font-semibold text-slate-700">
              Nota fijada (aparece en la cabecera y el POS)
            </label>
            <textarea
              maxLength={300}
              rows={2}
              value={pinnedNote}
              onChange={(e) => setPinnedNote(e.target.value)}
              placeholder="Instrucciones especiales, preferencias de entrega, etc."
              className="w-full rounded-xl border border-slate-200 px-3 py-2 text-xs font-medium text-slate-800 placeholder:text-slate-400 focus:border-teal-600 focus:outline-hidden"
            />
          </div>

          <DialogFooter className="gap-2 sm:gap-0 pt-2">
            <button
              type="button"
              onClick={() => onOpenChange(false)}
              className="rounded-xl border border-slate-200 px-4 py-2 text-xs font-semibold text-slate-700 hover:bg-slate-50 transition-colors"
            >
              Cancelar
            </button>
            <button
              type="submit"
              disabled={saving}
              className="flex items-center justify-center gap-2 rounded-xl bg-teal-700 px-5 py-2 text-xs font-semibold text-white hover:bg-teal-800 transition-colors disabled:opacity-50"
            >
              {saving ? <Loader2 className="size-4 animate-spin" /> : null}
              {customer ? "Guardar cambios" : "Crear cliente"}
            </button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
