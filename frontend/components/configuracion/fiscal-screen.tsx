"use client";

import { useState } from "react";
import { AlertTriangle, CheckCircle2, ExternalLink, Loader2, PlugZap, RotateCw, Save, Store } from "lucide-react";

import {
  api,
  friendlyError,
  type FiscalNumbering,
  type FiscalProvider,
  type FiscalSettings,
  type FiscalTestResult,
} from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { FISCAL_LABELS } from "@/components/fiscal/fiscal-status";
import { cn } from "@/lib/utils";

const MATIAS_SIGNUP = "https://sandbox-auth.matias-api.com/";

const EMPTY_NUMBERING: FiscalNumbering = {
  prefix: "",
  resolution_number: null,
  next_number: null,
  range_to: null,
  resolution_date: null,
  valid_until: null,
};

const input =
  "h-11 w-full rounded-xl border border-slate-200 bg-white px-3 text-sm outline-none focus:border-indigo-500 focus:ring-4 focus:ring-indigo-100 disabled:bg-slate-50 disabled:text-slate-500";

/**
 * Facturación electrónica por sede: proveedor (simulado o MATIAS), resolución
 * DIAN del POS electrónico y de las notas crédito, prueba de conexión y la
 * bandeja de documentos que la DIAN rechazó.
 */
export function FiscalScreen() {
  const currentBranch = useCurrentBranch();
  const branches = useSession((s) => s.branches);
  const [branchId, setBranchId] = useState<string>(currentBranch.id);
  const { data, loading, error, reload } = useApi(() => api.fiscal.settings(branchId), `fiscal:${branchId}`);

  return (
    <div className="space-y-6 p-6 lg:p-10">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">Facturación electrónica</h1>
          <p className="mt-1 max-w-2xl text-slate-500">
            Cada venta sale como documento equivalente POS electrónico y cada devolución como nota crédito. Nexa los
            envía a la DIAN apenas cobrás y reintenta solo si no hay internet.
          </p>
        </div>
        {branches.length > 1 && (
          <label className="flex flex-col gap-1 text-sm font-medium text-slate-700">
            Sede
            <span className="flex h-11 items-center gap-2 rounded-xl border border-slate-200 bg-white px-3">
              <Store className="size-4 text-slate-400" aria-hidden />
              <select value={branchId} onChange={(e) => setBranchId(e.target.value)} className="bg-transparent outline-none">
                {branches.map((b) => (
                  <option key={b.id} value={b.id}>
                    {b.name}
                  </option>
                ))}
              </select>
            </span>
          </label>
        )}
      </div>

      {loading && !data ? (
        <div className="flex h-64 items-center justify-center text-slate-500">
          <Loader2 className="size-6 animate-spin" aria-hidden />
          <span className="sr-only">Cargando</span>
        </div>
      ) : error || !data ? (
        <p className="rounded-xl bg-rose-50 p-3 text-sm text-rose-600">{friendlyError(error)}</p>
      ) : (
        <div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_380px]">
          <SettingsForm key={`${branchId}:${JSON.stringify(data)}`} initial={data} onSaved={reload} />
          <aside className="space-y-5">
            <ConnectionCard branchId={branchId} provider={data.provider} />
            <Steps />
          </aside>
          <div className="lg:col-span-2">
            <RejectedDocuments branchId={branchId} />
          </div>
        </div>
      )}
    </div>
  );
}

function SettingsForm({ initial, onSaved }: { initial: FiscalSettings; onSaved: () => void }) {
  const [s, setS] = useState<FiscalSettings>(initial);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ ok: boolean; text: string } | null>(null);
  const isMatias = s.provider === "MATIAS";
  const posMissing = !s.pos?.prefix.trim() || !s.pos?.resolution_number?.trim();

  function set<K extends keyof FiscalSettings>(key: K, value: FiscalSettings[K]) {
    setS((prev) => ({ ...prev, [key]: value }));
    setMessage(null);
  }

  async function save(e: React.FormEvent) {
    e.preventDefault();
    if (s.enabled && isMatias && posMissing) {
      setMessage({ ok: false, text: "Para enviar a la DIAN falta el prefijo y el número de resolución del POS." });
      return;
    }
    setSaving(true);
    setMessage(null);
    try {
      // El consecutivo solo se fija al crear la numeración; después lo lleva el backend.
      const clean = (n: FiscalNumbering | null, existing: FiscalNumbering | null) =>
        n && n.prefix.trim() ? { ...n, next_number: existing ? null : n.next_number } : null;
      await api.fiscal.saveSettings({
        ...s,
        pos: clean(s.pos, initial.pos),
        credit_notes: clean(s.credit_notes, initial.credit_notes),
      });
      setMessage({ ok: true, text: "Guardado." });
      onSaved();
    } catch (err) {
      setMessage({ ok: false, text: friendlyError(err) });
    } finally {
      setSaving(false);
    }
  }

  return (
    <form onSubmit={save} className="space-y-5">
      <section className="space-y-4 rounded-2xl border border-slate-200 bg-white p-5">
        <h2 className="text-base font-semibold">Proveedor</h2>
        <div className="grid gap-3 sm:grid-cols-2" role="radiogroup" aria-label="Proveedor">
          {(
            [
              ["SIMULADO", "Simulado", "Para probar la caja. No se envía nada a la DIAN."],
              ["MATIAS", "MATIAS API", "Proveedor tecnológico autorizado. Envía a la DIAN de verdad."],
            ] as [FiscalProvider, string, string][]
          ).map(([value, label, hint]) => (
            <button
              key={value}
              type="button"
              role="radio"
              aria-checked={s.provider === value}
              onClick={() => set("provider", value)}
              className={cn(
                "rounded-xl border p-4 text-left",
                s.provider === value ? "border-indigo-500 bg-indigo-50 ring-4 ring-indigo-100" : "border-slate-200",
              )}
            >
              <span className="block text-sm font-semibold">{label}</span>
              <span className="mt-1 block text-xs text-slate-500">{hint}</span>
            </button>
          ))}
        </div>
        <label className="flex items-center justify-between gap-4 rounded-xl bg-slate-50 p-4">
          <span>
            <span className="block text-sm font-semibold">Enviar documentos de esta sede</span>
            <span className="block text-xs text-slate-500">
              Apagado, las ventas quedan en cola y se envían cuando lo prendas.
            </span>
          </span>
          <input
            type="checkbox"
            checked={s.enabled}
            onChange={(e) => set("enabled", e.target.checked)}
            className="size-5 accent-indigo-600"
          />
        </label>
        <Field label="Dirección del punto de venta" hint="Sale en el documento. Si la dejás vacía se usa la de la sede.">
          <input
            className={input}
            value={s.pos_address ?? ""}
            onChange={(e) => set("pos_address", e.target.value || null)}
            maxLength={200}
          />
        </Field>
        {isMatias && (
          <Field
            label="ID de la empresa en MATIAS (opcional)"
            hint="Solo si tu cuenta de MATIAS es de casa de software y maneja varias empresas."
          >
            <input
              className={cn(input, "font-mono")}
              value={s.provider_client_id ?? ""}
              onChange={(e) => set("provider_client_id", e.target.value || null)}
              placeholder="00000000-0000-0000-0000-000000000000"
            />
          </Field>
        )}
      </section>

      <NumberingCard
        title="Resolución del POS electrónico"
        hint="La que te autorizó la DIAN para el documento equivalente POS. Está en MUISCA, en Numeración de facturación."
        value={s.pos}
        existing={initial.pos}
        onChange={(n) => set("pos", n)}
        withResolution
      />
      <NumberingCard
        title="Notas crédito (devoluciones)"
        hint="Las notas crédito no necesitan resolución, solo un prefijo propio, por ejemplo NC."
        value={s.credit_notes}
        existing={initial.credit_notes}
        onChange={(n) => set("credit_notes", n)}
      />

      <div className="flex flex-wrap items-center gap-3">
        <button
          type="submit"
          disabled={saving}
          className="flex h-12 items-center gap-2 rounded-xl bg-indigo-600 px-6 text-sm font-semibold text-white disabled:opacity-50"
        >
          {saving ? <Loader2 className="size-4 animate-spin" aria-hidden /> : <Save className="size-4" aria-hidden />}
          Guardar
        </button>
        {message && (
          <p role="status" className={cn("text-sm", message.ok ? "text-emerald-700" : "text-rose-700")}>
            {message.text}
          </p>
        )}
      </div>
    </form>
  );
}

function NumberingCard({
  title,
  hint,
  value,
  existing,
  onChange,
  withResolution = false,
}: {
  title: string;
  hint: string;
  value: FiscalNumbering | null;
  existing: FiscalNumbering | null;
  onChange: (n: FiscalNumbering) => void;
  withResolution?: boolean;
}) {
  const n = value ?? EMPTY_NUMBERING;
  const put = (patch: Partial<FiscalNumbering>) => onChange({ ...n, ...patch });
  const num = (v: string) => (v.trim() === "" ? null : Math.max(0, Math.floor(Number(v))) || null);
  const left = existing?.next_number != null && existing.range_to != null ? existing.range_to - existing.next_number + 1 : null;
  // Se calcula una vez al montar: la tarjeta no necesita reloj en vivo.
  const [now] = useState(() => Date.now());
  const expiresSoon = existing?.valid_until != null && new Date(existing.valid_until).getTime() - now < 30 * 86400000;

  return (
    <section className="space-y-4 rounded-2xl border border-slate-200 bg-white p-5">
      <div>
        <h2 className="text-base font-semibold">{title}</h2>
        <p className="mt-1 text-xs text-slate-500">{hint}</p>
      </div>
      <div className="grid gap-3 sm:grid-cols-3">
        <Field label="Prefijo">
          <input
            className={cn(input, "uppercase")}
            value={n.prefix}
            onChange={(e) => put({ prefix: e.target.value.toUpperCase().replace(/[^A-Z0-9]/g, "").slice(0, 4) })}
            placeholder={withResolution ? "POS" : "NC"}
          />
        </Field>
        {withResolution && (
          <Field label="Número de resolución">
            <input
              className={input}
              inputMode="numeric"
              value={n.resolution_number ?? ""}
              onChange={(e) => put({ resolution_number: e.target.value.replace(/\D/g, "") || null })}
            />
          </Field>
        )}
        <Field label={existing ? "Siguiente número" : "Empieza en"}>
          <input
            className={input}
            inputMode="numeric"
            value={existing ? (existing.next_number ?? "") : (n.next_number ?? "")}
            disabled={existing != null}
            onChange={(e) => put({ next_number: num(e.target.value) })}
            placeholder="1"
          />
        </Field>
        {withResolution && (
          <>
            <Field label="Rango hasta">
              <input
                className={input}
                inputMode="numeric"
                value={n.range_to ?? ""}
                onChange={(e) => put({ range_to: num(e.target.value) })}
              />
            </Field>
            <Field label="Fecha de la resolución">
              <input
                type="date"
                className={input}
                value={n.resolution_date ?? ""}
                onChange={(e) => put({ resolution_date: e.target.value || null })}
              />
            </Field>
            <Field label="Vigente hasta">
              <input
                type="date"
                className={input}
                value={n.valid_until ?? ""}
                onChange={(e) => put({ valid_until: e.target.value || null })}
              />
            </Field>
          </>
        )}
      </div>
      {withResolution && (left != null && left < 200 ? (
        <Warning>Quedan {Math.max(left, 0)} números en el rango. Pedí una resolución nueva en MUISCA.</Warning>
      ) : expiresSoon ? (
        <Warning>La resolución vence pronto. Pedí una nueva en MUISCA antes de que se acabe.</Warning>
      ) : null)}
    </section>
  );
}

function ConnectionCard({ branchId, provider }: { branchId: string; provider: FiscalProvider }) {
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<FiscalTestResult | { ok: false; message: string } | null>(null);

  async function test() {
    setBusy(true);
    setResult(null);
    try {
      setResult(await api.fiscal.test(branchId));
    } catch (e) {
      setResult({ ok: false, message: friendlyError(e) });
    } finally {
      setBusy(false);
    }
  }

  return (
    <div className="space-y-3 rounded-2xl border border-slate-200 bg-white p-5">
      <h2 className="text-base font-semibold">Conexión</h2>
      <p className="text-sm text-slate-500">
        {provider === "MATIAS"
          ? "Comprueba que el token de MATIAS del servidor es válido. Guardá antes de probar."
          : "En modo simulado no hay nada que conectar."}
      </p>
      <button
        type="button"
        onClick={test}
        disabled={busy}
        className="flex h-11 w-full items-center justify-center gap-2 rounded-xl bg-slate-900 text-sm font-semibold text-white disabled:opacity-50"
      >
        {busy ? <Loader2 className="size-4 animate-spin" aria-hidden /> : <PlugZap className="size-4" aria-hidden />}
        Probar conexión
      </button>
      {result && (
        <p
          role="status"
          className={cn(
            "flex items-start gap-2 rounded-xl p-3 text-sm",
            result.ok ? "bg-emerald-50 text-emerald-800" : "bg-rose-50 text-rose-700",
          )}
        >
          {result.ok ? (
            <CheckCircle2 className="mt-0.5 size-4 shrink-0" aria-hidden />
          ) : (
            <AlertTriangle className="mt-0.5 size-4 shrink-0" aria-hidden />
          )}
          {result.message}
        </p>
      )}
    </div>
  );
}

function Steps() {
  const steps = [
    <>
      Creá la cuenta de pruebas en{" "}
      <a href={MATIAS_SIGNUP} target="_blank" rel="noopener noreferrer" className="font-semibold text-indigo-600 hover:underline">
        MATIAS <ExternalLink className="inline size-3" aria-hidden />
      </a>{" "}
      y registrá tu empresa (NIT, régimen, dirección).
    </>,
    <>En MATIAS creá un token de acceso y pegalo en el servidor como MATIAS_API_TOKEN. Nunca lo mandes por chat.</>,
    <>En MUISCA (DIAN) habilitate como facturador y pedí la numeración del POS electrónico.</>,
    <>Cargá acá el prefijo y la resolución, elegí MATIAS y prendé el envío.</>,
    <>Probá la conexión y hacé una venta de prueba: el tiquete sale con el CUDE y el QR.</>,
  ];
  return (
    <div className="space-y-3 rounded-2xl border border-slate-200 bg-white p-5">
      <h2 className="text-base font-semibold">Para empezar a facturar</h2>
      <ol className="space-y-2 text-sm text-slate-600">
        {steps.map((step, i) => (
          <li key={i} className="flex gap-3">
            <span className="flex size-6 shrink-0 items-center justify-center rounded-full bg-indigo-50 text-xs font-bold text-indigo-700">
              {i + 1}
            </span>
            <span>{step}</span>
          </li>
        ))}
      </ol>
    </div>
  );
}

function RejectedDocuments({ branchId }: { branchId: string }) {
  const { data, loading, error, reload } = useApi(
    () => api.fiscal.documents(branchId, "RECHAZADO"),
    `fiscal-docs:${branchId}`,
  );
  const [busy, setBusy] = useState<string | null>(null);
  const [retryError, setRetryError] = useState<string | null>(null);

  async function retry(id: string) {
    setBusy(id);
    setRetryError(null);
    try {
      await api.fiscal.retry(id);
      await reload();
    } catch (e) {
      setRetryError(friendlyError(e));
    } finally {
      setBusy(null);
    }
  }

  return (
    <section className="space-y-3 rounded-2xl border border-slate-200 bg-white p-5">
      <div className="flex items-center justify-between gap-4">
        <h2 className="text-base font-semibold">Rechazados por la DIAN</h2>
        <button
          type="button"
          onClick={() => void reload()}
          className="flex h-9 items-center gap-1.5 rounded-full border border-slate-200 px-3 text-xs font-semibold text-slate-700"
        >
          <RotateCw className="size-3.5" aria-hidden /> Actualizar
        </button>
      </div>
      {loading && !data ? (
        <Loader2 className="size-5 animate-spin text-slate-400" aria-hidden />
      ) : error ? (
        <p className="text-sm text-rose-600">{friendlyError(error)}</p>
      ) : !data || data.length === 0 ? (
        <p className="text-sm text-slate-500">Ninguno. Todo lo que se envió fue aceptado o está en camino.</p>
      ) : (
        <ul className="divide-y divide-slate-100">
          {data.map((d) => (
            <li key={d.id} className="flex flex-wrap items-start justify-between gap-3 py-3">
              <div className="min-w-0 flex-1">
                <p className="text-sm font-semibold">
                  {d.doc_type === "DEVOLUCION" ? "Nota crédito" : "POS"} {d.prefix}-{d.number}{" "}
                  <span className="font-normal text-slate-500">
                    · {FISCAL_LABELS[d.status] ?? d.status} · {d.attempts} intentos ·{" "}
                    {new Date(d.created_at).toLocaleString("es-CO", { hour12: false })}
                  </span>
                </p>
                {d.last_error && <p className="mt-1 text-sm break-words text-rose-700">{d.last_error}</p>}
              </div>
              <button
                type="button"
                onClick={() => retry(d.id)}
                disabled={busy === d.id}
                className="flex h-9 items-center gap-1.5 rounded-full bg-slate-900 px-3 text-xs font-semibold text-white disabled:opacity-50"
              >
                {busy === d.id ? (
                  <Loader2 className="size-3.5 animate-spin" aria-hidden />
                ) : (
                  <RotateCw className="size-3.5" aria-hidden />
                )}
                Reintentar
              </button>
            </li>
          ))}
        </ul>
      )}
      {retryError && <p className="text-sm text-rose-700">{retryError}</p>}
    </section>
  );
}

function Field({ label, hint, children }: { label: string; hint?: string; children: React.ReactNode }) {
  return (
    <label className="block space-y-1">
      <span className="block text-sm font-medium text-slate-700">{label}</span>
      {children}
      {hint && <span className="block text-xs text-slate-500">{hint}</span>}
    </label>
  );
}

function Warning({ children }: { children: React.ReactNode }) {
  return (
    <p className="flex items-start gap-2 rounded-xl bg-amber-50 p-3 text-sm text-amber-800">
      <AlertTriangle className="mt-0.5 size-4 shrink-0" aria-hidden />
      {children}
    </p>
  );
}
