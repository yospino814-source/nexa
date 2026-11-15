"use client";

import { useCallback, useEffect, useState } from "react";
import { AlertTriangle, CheckCircle2, Clock, Loader2, RotateCw } from "lucide-react";

import { api, friendlyError, type FiscalStatus, type OrderFiscalDocument } from "@/lib/api";
import { cn } from "@/lib/utils";

export const FISCAL_LABELS: Record<FiscalStatus, string> = {
  PENDIENTE: "Por enviar a la DIAN",
  ENVIADO: "Enviado, esperando la DIAN",
  EN_PROCESO: "La DIAN lo está validando",
  ACEPTADO: "Validado por la DIAN",
  RECHAZADO: "Rechazado",
  ANULADO: "Anulado",
};

const WAITING: FiscalStatus[] = ["PENDIENTE", "ENVIADO", "EN_PROCESO"];

/**
 * Documento electrónico de una venta. Mientras la DIAN no responde consulta
 * cada 4 s (el backend lo envía apenas se cobra), y se detiene a los 2 min:
 * después el trabajador sigue reintentando solo y basta con reabrir la venta.
 */
export function useOrderFiscal(orderId: string | null) {
  const [doc, setDoc] = useState<OrderFiscalDocument | null>(null);
  const [loaded, setLoaded] = useState(false);
  // Cambiarlo vuelve a consultar y reinicia la espera (por ejemplo tras un reintento).
  const [round, setRound] = useState(0);

  useEffect(() => {
    if (!orderId) return;
    let alive = true;
    let timer: ReturnType<typeof setTimeout> | undefined;
    const started = Date.now();
    const tick = async () => {
      let d: OrderFiscalDocument | null = null;
      try {
        d = await api.fiscal.orderDocument(orderId);
      } catch {
        // Sin conexión o sin permiso: el detalle de la venta sigue sirviendo.
      }
      if (!alive) return;
      setDoc(d);
      setLoaded(true);
      if (d && WAITING.includes(d.status) && Date.now() - started < 120_000) {
        timer = setTimeout(tick, 4000);
      }
    };
    void tick();
    return () => {
      alive = false;
      if (timer) clearTimeout(timer);
    };
  }, [orderId, round]);

  const reload = useCallback(() => setRound((r) => r + 1), []);
  return { doc, loaded, reload };
}

/** Estado del documento electrónico con su detalle y el botón de reintento. */
export function FiscalStatusPanel({
  doc,
  canRetry,
  onRetried,
}: {
  doc: OrderFiscalDocument;
  canRetry: boolean;
  onRetried: () => void;
}) {
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const waiting = WAITING.includes(doc.status);
  const rejected = doc.status === "RECHAZADO";
  const simulated = doc.provider === "SIMULADO";

  async function retry() {
    setBusy(true);
    setError(null);
    try {
      await api.fiscal.retry(doc.id);
      onRetried();
    } catch (e) {
      setError(friendlyError(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div
      className={cn(
        "space-y-2 rounded-2xl p-3 text-sm",
        rejected ? "bg-rose-50" : doc.status === "ACEPTADO" ? "bg-emerald-50" : "bg-slate-50",
      )}
    >
      <div className="flex items-start gap-2">
        {doc.status === "ACEPTADO" ? (
          <CheckCircle2 className="mt-0.5 size-4 shrink-0 text-emerald-600" aria-hidden />
        ) : rejected ? (
          <AlertTriangle className="mt-0.5 size-4 shrink-0 text-rose-600" aria-hidden />
        ) : waiting ? (
          <Loader2 className="mt-0.5 size-4 shrink-0 animate-spin text-slate-500" aria-hidden />
        ) : (
          <Clock className="mt-0.5 size-4 shrink-0 text-slate-500" aria-hidden />
        )}
        <div className="min-w-0 flex-1">
          <p className="font-semibold text-slate-900">
            {doc.number} · {FISCAL_LABELS[doc.status] ?? doc.status}
            {simulated && <span className="font-normal text-slate-500"> (modo simulado)</span>}
          </p>
          {doc.cude && <p className="mt-0.5 font-mono text-[11px] break-all text-slate-500">CUDE {doc.cude}</p>}
          {doc.last_error && !waiting && doc.status !== "ACEPTADO" && (
            <p className="mt-1 text-rose-700">{doc.last_error}</p>
          )}
        </div>
      </div>
      {(rejected || (doc.pdf_url && doc.status === "ACEPTADO")) && (
        <div className="flex flex-wrap gap-2 pl-6">
          {rejected && canRetry && (
            <button
              type="button"
              onClick={retry}
              disabled={busy}
              className="flex h-9 items-center gap-1.5 rounded-full bg-slate-900 px-3 text-xs font-semibold text-white disabled:opacity-50"
            >
              {busy ? <Loader2 className="size-3.5 animate-spin" aria-hidden /> : <RotateCw className="size-3.5" aria-hidden />}
              Corregí los datos, reintentar
            </button>
          )}
          {doc.pdf_url && doc.status === "ACEPTADO" && (
            <a
              href={doc.pdf_url}
              target="_blank"
              rel="noopener noreferrer"
              className="flex h-9 items-center rounded-full border border-slate-200 bg-white px-3 text-xs font-semibold text-slate-800"
            >
              Ver PDF
            </a>
          )}
        </div>
      )}
      {error && <p className="pl-6 text-rose-700">{error}</p>}
    </div>
  );
}
