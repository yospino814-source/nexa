"use client";

import { useEffect, useMemo, useState } from "react";
import Link from "next/link";
import { Bike, Check, Loader2, MapPin, QrCode, RefreshCw, Store, X } from "lucide-react";

import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { Nexi } from "@/components/nexa/nexi";
import { api, friendlyError, type WaOrder, type WaOrderStatus } from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { formatCOP } from "@/lib/format";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { cn } from "@/lib/utils";

const COLUMNS: { status: WaOrderStatus; label: string; dot: string }[] = [
  { status: "NUEVO", label: "Nuevos", dot: "bg-indigo-600" },
  { status: "PREPARANDO", label: "Preparando", dot: "bg-amber-500" },
  { status: "EN_CAMINO", label: "En camino", dot: "bg-grad-violet" },
  { status: "ENTREGADO", label: "Entregados", dot: "bg-emerald-500" },
];

const NEXT: Partial<Record<WaOrderStatus, { to: WaOrderStatus; label: string }>> = {
  NUEVO: { to: "PREPARANDO", label: "Empezar a preparar" },
  PREPARANDO: { to: "EN_CAMINO", label: "Despachar" },
  EN_CAMINO: { to: "ENTREGADO", label: "Marcar entregado" },
};

function ago(iso: string, now: number): string {
  const min = Math.max(0, Math.round((now - new Date(iso).getTime()) / 60000));
  if (min < 1) return "ahora";
  if (min < 60) return `hace ${min} min`;
  const h = Math.floor(min / 60);
  return `hace ${h} h`;
}

function payment(o: WaOrder): { label: string; tone: "ok" | "warn" | "mute" } {
  if (o.payment_method === "LINK") {
    return o.payment_status === "PAGADO"
      ? { label: "Pagado por link", tone: "ok" }
      : { label: "Link sin pagar", tone: "warn" };
  }
  if (o.status === "ENTREGADO") return { label: o.payment_method === "DATAFONO" ? "Datáfono" : "Efectivo", tone: "ok" };
  return { label: o.payment_method === "DATAFONO" ? "Datáfono al recibir" : "Efectivo al recibir", tone: "mute" };
}

function shipping(o: WaOrder): string {
  if (o.pickup) return "Recoge en tienda";
  if (o.status === "EN_CAMINO" && o.courier_name) return `Con ${o.courier_name}`;
  return o.zone_name ? `Domicilio · ${o.zone_name}` : "Domicilio";
}

const BADGE = {
  ok: "bg-emerald-50 text-emerald-700",
  warn: "bg-amber-50 text-amber-700",
  mute: "bg-slate-100 text-slate-600",
};

/**
 * Bandeja de pedidos por WhatsApp: Nexi los toma en el chat y acá se
 * preparan, despachan y entregan. Cada cambio le avisa al cliente.
 */
export function WhatsappOrdersScreen() {
  const branch = useCurrentBranch();
  const [round, setRound] = useState(0);
  const { data, loading, error } = useApi(() => api.whatsappOrders.list(branch.id), `${branch.id}:${round}`);
  const [openId, setOpenId] = useState<string | null>(null);
  const [now, setNow] = useState(() => Date.now());

  // Llegan pedidos nuevos sin recargar.
  useEffect(() => {
    const t = setInterval(() => {
      setRound((r) => r + 1);
      setNow(Date.now());
    }, 20000);
    return () => clearInterval(t);
  }, []);

  const orders = useMemo(() => data ?? [], [data]);
  const today = orders.filter((o) => o.status !== "CANCELADO");
  const totalToday = today.reduce((s, o) => s + Number(o.total), 0);
  const fresh = orders.filter((o) => o.status === "NUEVO").length;

  return (
    <div className="mx-auto flex w-full max-w-[1280px] flex-col gap-6 px-6 pt-7 pb-14 lg:px-10">
      <section className="relative overflow-hidden rounded-2xl bg-carbon px-6 py-6 text-white shadow-card">
        <span
          aria-hidden
          className="pointer-events-none absolute -top-24 -right-16 size-72 rounded-full bg-[radial-gradient(circle,rgba(108,62,245,.5),transparent_70%)]"
        />
        <div className="relative flex flex-wrap items-end justify-between gap-4">
          <div className="flex items-center gap-4">
            <Nexi expr={fresh > 0 ? "alert" : "happy"} size={72} animated />
            <div>
              <p className="text-[13px] font-medium text-slate-400">Pedidos por WhatsApp</p>
              <h1 className="mt-0.5 text-2xl font-semibold tracking-[-0.022em] tabular-nums">
                {today.length} {today.length === 1 ? "pedido" : "pedidos"} · {formatCOP(totalToday)}
              </h1>
              <p className="mt-1 text-sm text-slate-300">
                Nexi toma el pedido en el chat; vos lo preparás, lo despachás y se cobra en la caja.
              </p>
            </div>
          </div>
          <div className="flex flex-wrap items-center gap-2.5">
            <button
              type="button"
              onClick={() => setRound((r) => r + 1)}
              className="flex h-10 items-center gap-2 rounded-[10px] bg-white/10 px-4 text-sm font-semibold hover:bg-white/15"
            >
              <RefreshCw className={cn("size-4", loading && "animate-spin")} aria-hidden /> Actualizar
            </button>
            <Link
              href="/configuracion/whatsapp"
              className="flex h-10 items-center gap-2 rounded-[10px] bg-white px-4 text-sm font-semibold text-slate-900 hover:bg-slate-100"
            >
              <QrCode className="size-4" aria-hidden /> QR de la tienda
            </Link>
          </div>
        </div>
      </section>

      {error && <p className="rounded-xl bg-rose-50 p-3 text-sm text-rose-700">{friendlyError(error)}</p>}

      <div className="grid gap-4 md:grid-cols-2 xl:grid-cols-4">
        {COLUMNS.map((col) => {
          const list = orders.filter((o) => o.status === col.status);
          return (
            <section
              key={col.status}
              aria-label={col.label}
              className="flex min-w-0 flex-col gap-2.5 rounded-2xl border border-slate-200 bg-[#F8F9FC] p-3.5"
            >
              <h2 className="flex items-center gap-2 px-1 py-0.5 text-sm font-semibold">
                <span className={cn("size-[7px] rounded-full", col.dot)} />
                {col.label}
                <span className="ml-auto font-medium text-slate-500 tabular-nums">{list.length}</span>
              </h2>
              {loading && !data ? (
                <div className="h-28 animate-pulse rounded-2xl bg-slate-200/60" />
              ) : list.length === 0 ? (
                <p className="px-1 py-6 text-center text-xs text-slate-500">Sin pedidos</p>
              ) : (
                list.map((o) => {
                  const pay = payment(o);
                  const units = o.items.reduce((s, i) => s + i.qty, 0);
                  return (
                    <button
                      key={o.id}
                      type="button"
                      onClick={() => setOpenId(o.id)}
                      className="nx-rise flex w-full flex-col gap-2 rounded-2xl border border-slate-200 bg-white p-3.5 text-left shadow-[0_1px_2px_rgb(20_22_28/0.05)] transition-transform hover:-translate-y-0.5 hover:border-indigo-200 active:scale-[0.98]"
                    >
                      <span className="flex items-center justify-between gap-2 text-xs tabular-nums">
                        <span className="font-bold text-slate-500">{o.code}</span>
                        <span className="text-slate-400">{ago(o.created_at, now)}</span>
                      </span>
                      <span className="text-[15px] font-semibold">{o.contact_name ?? o.wa_phone}</span>
                      <span className="flex items-baseline justify-between gap-2">
                        <span className="text-[13px] text-slate-500">
                          {units} {units === 1 ? "producto" : "productos"}
                        </span>
                        <span className="font-bold tabular-nums">{formatCOP(Number(o.total))}</span>
                      </span>
                      <span className="flex flex-wrap gap-1.5">
                        <span className={cn("inline-flex h-[22px] items-center rounded-full px-2 text-xs font-semibold", BADGE[pay.tone])}>
                          {pay.label}
                        </span>
                        <span className="inline-flex h-[22px] items-center rounded-full bg-slate-100 px-2 text-xs font-semibold text-slate-600">
                          {shipping(o)}
                        </span>
                      </span>
                    </button>
                  );
                })
              )}
            </section>
          );
        })}
      </div>

      <OrderDialog
        orderId={openId}
        onClose={() => setOpenId(null)}
        onChanged={() => setRound((r) => r + 1)}
      />
    </div>
  );
}

function OrderDialog({
  orderId,
  onClose,
  onChanged,
}: {
  orderId: string | null;
  onClose: () => void;
  onChanged: () => void;
}) {
  return (
    <Dialog open={orderId !== null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="max-h-[92vh] gap-0 overflow-y-auto rounded-[20px] p-0 shadow-pop sm:max-w-[640px]">
        {orderId && <OrderDetail key={orderId} orderId={orderId} onChanged={onChanged} onClose={onClose} />}
      </DialogContent>
    </Dialog>
  );
}

function OrderDetail({ orderId, onChanged, onClose }: { orderId: string; onChanged: () => void; onClose: () => void }) {
  const can = useSession((s) => s.can);
  const [version, setVersion] = useState(0);
  const { data, loading, error } = useApi(() => api.whatsappOrders.detail(orderId), `${orderId}:${version}`);
  const [busy, setBusy] = useState(false);
  const [actionError, setActionError] = useState<string | null>(null);
  const [courier, setCourier] = useState("");
  const [reference, setReference] = useState("");
  const [confirmCancel, setConfirmCancel] = useState(false);

  if (loading && !data) {
    return <Loader2 className="mx-auto my-16 size-6 animate-spin text-slate-400" aria-label="Cargando" />;
  }
  if (error || !data) {
    return <p className="m-6 rounded-xl bg-rose-50 p-3 text-sm text-rose-700">{friendlyError(error)}</p>;
  }

  const o = data.order;
  const next = NEXT[o.status];
  const needsCourier = next?.to === "EN_CAMINO" && !o.pickup;
  const linkUnpaid = o.payment_method === "LINK" && o.payment_status !== "PAGADO";
  const blockedByPayment = next?.to === "EN_CAMINO" && linkUnpaid;
  const canCancel = can("pos:refund") && ["NUEVO", "PREPARANDO", "EN_CAMINO"].includes(o.status);

  async function run(fn: () => Promise<unknown>) {
    setBusy(true);
    setActionError(null);
    try {
      await fn();
      setVersion((v) => v + 1);
      onChanged();
    } catch (e) {
      setActionError(friendlyError(e));
    } finally {
      setBusy(false);
    }
  }

  return (
    <div>
      <div className="flex items-start justify-between gap-4 pt-6 pr-14 pl-6">
        <div>
          <p className="text-xs font-bold text-slate-500 tabular-nums">{o.code}</p>
          <DialogTitle className="text-xl font-semibold tracking-[-0.01em]">{o.contact_name ?? o.wa_phone}</DialogTitle>
          <p className="mt-0.5 text-[13px] text-slate-500 tabular-nums">+{o.wa_phone}</p>
        </div>
        <span className={cn("mt-1 inline-flex h-6 items-center rounded-full px-2.5 text-xs font-semibold", BADGE[payment(o).tone])}>
          {payment(o).label}
        </span>
      </div>

      <div className="flex flex-col gap-4 px-6 py-5">
        <div className="flex items-start gap-2.5 rounded-xl bg-slate-50 p-3 text-sm">
          {o.pickup ? (
            <Store className="mt-0.5 size-4 shrink-0 text-slate-500" aria-hidden />
          ) : (
            <MapPin className="mt-0.5 size-4 shrink-0 text-slate-500" aria-hidden />
          )}
          <span>
            {o.pickup ? "Recoge en la tienda" : [o.address, o.zone_name].filter(Boolean).join(" · ") || "Domicilio"}
            {o.age_confirmed && <span className="block text-xs text-slate-500">Confirmó que es mayor de 18. Pedile la cédula al entregar.</span>}
          </span>
        </div>

        <div className="overflow-hidden rounded-xl border border-slate-200">
          <table className="w-full text-sm">
            <tbody className="tabular-nums">
              {o.items.map((i, idx) => (
                <tr key={idx} className="border-b border-slate-100 last:border-0">
                  <td className="px-3.5 py-2.5">{i.name}</td>
                  <td className="px-2 py-2.5 text-right text-slate-500">×{i.qty}</td>
                  <td className="px-3.5 py-2.5 text-right font-medium">{formatCOP(Number(i.unit_price) * i.qty)}</td>
                </tr>
              ))}
            </tbody>
          </table>
          <dl className="space-y-1 border-t border-slate-200 bg-[#FBFBFD] px-3.5 py-3 text-sm tabular-nums">
            {Number(o.delivery_fee) > 0 && (
              <div className="flex justify-between text-slate-500">
                <dt>Domicilio</dt>
                <dd>{formatCOP(Number(o.delivery_fee))}</dd>
              </div>
            )}
            <div className="flex justify-between text-base font-bold">
              <dt>Total</dt>
              <dd>{formatCOP(Number(o.total))}</dd>
            </div>
          </dl>
        </div>

        {data.messages.length > 0 && (
          <details className="group rounded-xl bg-[#ECE5DD] p-3">
            <summary className="cursor-pointer text-[13px] font-semibold text-slate-700">Ver la conversación con Nexi</summary>
            <ul className="mt-3 flex flex-col gap-1.5">
              {data.messages.map((m, i) => (
                <li
                  key={i}
                  className={cn(
                    "max-w-[84%] px-2.5 py-1.5 text-[13px] leading-snug whitespace-pre-line shadow-[0_1px_1px_rgba(0,0,0,.08)]",
                    m.direction === "IN"
                      ? "self-end rounded-[14px_4px_14px_14px] bg-[#D9FDD3]"
                      : "self-start rounded-[4px_14px_14px_14px] bg-white",
                  )}
                >
                  {m.body}
                </li>
              ))}
            </ul>
          </details>
        )}

        {linkUnpaid && o.status !== "CANCELADO" && can("pos:sell") && (
          <form
            className="flex flex-wrap items-end gap-2 rounded-xl bg-amber-50 p-3 ring-1 ring-amber-200 ring-inset"
            onSubmit={(e) => {
              e.preventDefault();
              void run(() => api.whatsappOrders.markPaid(o.id, reference.trim()));
            }}
          >
            <label className="flex min-w-0 flex-1 flex-col gap-1.5 text-[13px] font-medium text-amber-900">
              ¿Ya pagó? Escribí la referencia del pago
              <input
                value={reference}
                onChange={(e) => setReference(e.target.value)}
                placeholder="Ej. NEQUI-123456"
                className="h-10 rounded-[10px] border border-amber-200 bg-white px-3 text-sm text-slate-900 outline-none focus:border-indigo-600 focus:ring-4 focus:ring-indigo-600/18"
              />
            </label>
            <button
              type="submit"
              disabled={busy || reference.trim().length < 3}
              className="h-10 rounded-[10px] border border-slate-300 bg-white px-4 text-sm font-semibold shadow-xs disabled:opacity-50"
            >
              Marcar pagado
            </button>
          </form>
        )}

        {needsCourier && !blockedByPayment && (
          <label className="flex flex-col gap-1.5 text-[13px] font-medium text-slate-700">
            ¿Quién lo lleva?
            <input
              value={courier}
              onChange={(e) => setCourier(e.target.value)}
              placeholder="Nombre del domiciliario"
              className="h-11 rounded-[10px] border border-slate-300 bg-white px-3 text-sm outline-none focus:border-indigo-600 focus:ring-4 focus:ring-indigo-600/18"
            />
            <span className="text-xs font-normal text-slate-500">Nexi le avisa al cliente quién va en camino.</span>
          </label>
        )}

        {actionError && <p className="rounded-xl bg-rose-50 p-3 text-sm text-rose-700">{actionError}</p>}
      </div>

      <div className="flex flex-wrap items-center justify-between gap-2.5 border-t border-slate-100 px-6 py-4">
        {canCancel ? (
          confirmCancel ? (
            <span className="flex items-center gap-2 text-sm">
              ¿Cancelar el pedido?
              <button
                type="button"
                disabled={busy}
                onClick={() => run(() => api.whatsappOrders.setStatus(o.id, "CANCELADO")).then(onClose)}
                className="h-9 rounded-lg bg-rose-600 px-3 text-[13px] font-semibold text-white hover:bg-rose-700"
              >
                Sí, cancelar
              </button>
              <button type="button" onClick={() => setConfirmCancel(false)} className="h-9 rounded-lg px-3 text-[13px] font-semibold text-slate-600">
                No
              </button>
            </span>
          ) : (
            <button
              type="button"
              onClick={() => setConfirmCancel(true)}
              className="flex h-10 items-center gap-1.5 rounded-[10px] px-3 text-sm font-semibold text-rose-600 hover:bg-rose-50"
            >
              <X className="size-4" aria-hidden /> Cancelar pedido
            </button>
          )
        ) : (
          <span />
        )}
        {next && (
          <button
            type="button"
            disabled={busy || blockedByPayment || (needsCourier && !courier.trim())}
            title={blockedByPayment ? "El pedido tiene link de pago y todavía no está pagado" : undefined}
            onClick={() => run(() => api.whatsappOrders.setStatus(o.id, next.to, needsCourier ? courier.trim() : null))}
            className="flex h-11 items-center gap-2 rounded-xl bg-indigo-600 px-5 text-[15px] font-semibold text-white shadow-[0_1px_2px_rgb(20_22_28/0.12),inset_0_1px_0_rgb(255_255_255/0.14)] hover:bg-indigo-700 disabled:opacity-45"
          >
            {busy ? (
              <Loader2 className="size-4 animate-spin" aria-hidden />
            ) : next.to === "EN_CAMINO" ? (
              <Bike className="size-4" aria-hidden />
            ) : (
              <Check className="size-4" aria-hidden />
            )}
            {next.to === "EN_CAMINO" && o.pickup ? "Avisar que está listo" : next.label}
          </button>
        )}
      </div>
    </div>
  );
}
