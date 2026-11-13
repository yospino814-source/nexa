"use client";

import { useEffect, useState } from "react";
import { formatCOP } from "@/lib/format";
import { api, type BusinessInfo, type CustomerStatementResponse } from "@/lib/api";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { PrintArea } from "@/components/nexa/print-area";
import { Loader2, Printer } from "lucide-react";

function getFirstDayOfPreviousMonth(): string {
  const now = new Date();
  const prevMonth = new Date(now.getFullYear(), now.getMonth() - 1, 1);
  const y = prevMonth.getFullYear();
  const m = String(prevMonth.getMonth() + 1).padStart(2, "0");
  return `${y}-${m}-01`;
}

function getTodayString(): string {
  return new Date().toISOString().slice(0, 10);
}

function formatDateDMY(d: string): string {
  const parts = d.slice(0, 10).split("-");
  if (parts.length === 3) {
    return `${parts[2].padStart(2, "0")}/${parts[1].padStart(2, "0")}/${parts[0]}`;
  }
  return d;
}

export type StatementDialogProps = {
  open: boolean;
  onClose: () => void;
  customerId: string;
  customerName: string;
  customerDoc?: string | null;
  ticketWidthMm?: 58 | 80;
};

export function StatementDialog({
  open,
  onClose,
  customerId,
  customerName,
  customerDoc,
  ticketWidthMm = 80,
}: StatementDialogProps) {
  const [from, setFrom] = useState(getFirstDayOfPreviousMonth);
  const [to, setTo] = useState(getTodayString);
  const [format, setFormat] = useState<"tirilla" | "carta">("tirilla");
  const [statement, setStatement] = useState<CustomerStatementResponse | null>(null);
  const [loading, setLoading] = useState(false);
  const [business, setBusiness] = useState<BusinessInfo | null>(null);

  useEffect(() => {
    let active = true;
    api.business.get().then((info) => {
      if (active) setBusiness(info);
    }).catch(() => {});
    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (!open || !customerId) return;
    let cancelled = false;
    Promise.resolve().then(() => {
      if (!cancelled) setLoading(true);
      return api.customers.statement(customerId, from, to);
    }).then((data) => {
      if (!cancelled) {
        setStatement(data);
        setLoading(false);
      }
    }).catch((err) => {
      if (!cancelled) {
        console.error("Error cargando estado de cuenta:", err);
        setLoading(false);
      }
    });

    return () => {
      cancelled = true;
    };
  }, [open, customerId, from, to]);

  const handlePrint = () => {
    window.print();
  };

  const businessName = business?.business_name ?? business?.name ?? "Nexa POS";
  const nit = business?.nit ? `NIT ${business.nit}${business.dv ? `-${business.dv}` : ""}` : null;

  return (
    <>
      <Dialog open={open} onOpenChange={(val) => !val && onClose()}>
        <DialogContent className="max-w-3xl max-h-[90vh] flex flex-col">
          <DialogHeader>
            <DialogTitle>Estado de cuenta: {customerName}</DialogTitle>
          </DialogHeader>

          {/* Filtros de fecha y formato */}
          <div className="flex flex-wrap items-center justify-between gap-3 border-b pb-3 text-xs">
            <div className="flex items-center gap-2">
              <label className="font-semibold text-slate-700">Desde:</label>
              <input
                type="date"
                value={from}
                onChange={(e) => setFrom(e.target.value)}
                className="rounded-lg border border-slate-200 px-2.5 py-1 text-xs outline-none focus:border-teal-600 focus:ring-1 focus:ring-teal-600"
              />
              <label className="font-semibold text-slate-700 ml-2">Hasta:</label>
              <input
                type="date"
                value={to}
                onChange={(e) => setTo(e.target.value)}
                className="rounded-lg border border-slate-200 px-2.5 py-1 text-xs outline-none focus:border-teal-600 focus:ring-1 focus:ring-teal-600"
              />
            </div>

            <div className="flex items-center gap-2">
              <span className="font-semibold text-slate-700">Formato:</span>
              <div className="inline-flex rounded-lg border border-slate-200 p-0.5 bg-slate-50">
                <button
                  type="button"
                  onClick={() => setFormat("tirilla")}
                  className={`px-2.5 py-1 rounded-md font-semibold transition-colors ${
                    format === "tirilla"
                      ? "bg-white text-teal-800 shadow-xs"
                      : "text-slate-600 hover:text-slate-900"
                  }`}
                >
                  Tirilla
                </button>
                <button
                  type="button"
                  onClick={() => setFormat("carta")}
                  className={`px-2.5 py-1 rounded-md font-semibold transition-colors ${
                    format === "carta"
                      ? "bg-white text-teal-800 shadow-xs"
                      : "text-slate-600 hover:text-slate-900"
                  }`}
                >
                  Carta
                </button>
              </div>
            </div>
          </div>

          {/* Resumen de saldos */}
          {statement && (
            <div className="grid grid-cols-2 gap-3 py-2">
              <div className="rounded-xl border border-slate-100 bg-slate-50 p-3">
                <span className="text-[11px] font-bold uppercase tracking-wider text-slate-400">
                  Saldo inicial (anterior a {formatDateDMY(from)})
                </span>
                <p className="text-base font-bold text-slate-700 mt-0.5">
                  {formatCOP(Number(statement.opening_balance))}
                </p>
              </div>
              <div className="rounded-xl border border-teal-100 bg-teal-50/50 p-3">
                <span className="text-[11px] font-bold uppercase tracking-wider text-teal-700">
                  Saldo final (al {formatDateDMY(to)})
                </span>
                <p className="text-base font-bold text-teal-900 mt-0.5">
                  {formatCOP(Number(statement.closing_balance))}
                </p>
              </div>
            </div>
          )}

          {/* Tabla de movimientos */}
          <div className="flex-1 overflow-auto rounded-xl border border-slate-100 min-h-[220px]">
            {loading ? (
              <div className="flex h-full items-center justify-center p-8 text-slate-400">
                <Loader2 className="size-5 animate-spin mr-2 text-teal-600" />
                Cargando estado de cuenta...
              </div>
            ) : !statement || statement.entries.length === 0 ? (
              <div className="flex h-full items-center justify-center p-8 text-center text-xs text-slate-400">
                No hay movimientos registrados en el período seleccionado.
              </div>
            ) : (
              <table className="w-full text-left text-xs">
                <thead className="sticky top-0 bg-slate-50 text-[11px] font-bold uppercase tracking-wider text-slate-400 border-b">
                  <tr>
                    <th className="py-2 px-3">Fecha</th>
                    <th className="py-2 px-3">Tipo</th>
                    <th className="py-2 px-3">Referencia</th>
                    <th className="py-2 px-3 text-right">Cargo</th>
                    <th className="py-2 px-3 text-right">Abono</th>
                    <th className="py-2 px-3 text-right">Saldo</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-100">
                  {statement.entries.map((entry, idx) => {
                    const isDebit = Number(entry.debit) > 0;
                    const isCredit = Number(entry.credit) > 0;
                    return (
                      <tr key={idx} className="hover:bg-slate-50/80">
                        <td className="py-2 px-3 text-slate-600 whitespace-nowrap">
                          {formatDateDMY(entry.date)}
                        </td>
                        <td className="py-2 px-3 font-semibold text-slate-700">
                          <span
                            className={`inline-block px-1.5 py-0.5 rounded text-[10px] font-bold ${
                              entry.type === "CARGO"
                                ? "bg-amber-50 text-amber-700 border border-amber-200"
                                : entry.type === "DEVOLUCION"
                                  ? "bg-purple-50 text-purple-700 border border-purple-200"
                                  : "bg-teal-50 text-teal-700 border border-teal-200"
                            }`}
                          >
                            {entry.type}
                          </span>
                        </td>
                        <td className="py-2 px-3 text-slate-600 font-medium">
                          {entry.reference}
                        </td>
                        <td className="py-2 px-3 text-right font-medium text-slate-800">
                          {isDebit ? formatCOP(Number(entry.debit)) : "-"}
                        </td>
                        <td className="py-2 px-3 text-right font-medium text-emerald-700">
                          {isCredit ? formatCOP(Number(entry.credit)) : "-"}
                        </td>
                        <td className="py-2 px-3 text-right font-bold text-slate-900">
                          {formatCOP(Number(entry.balance))}
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            )}
          </div>

          <DialogFooter className="flex sm:justify-between gap-2 pt-3 border-t">
            <Button type="button" variant="outline" onClick={onClose}>
              Cerrar
            </Button>
            <Button
              type="button"
              onClick={handlePrint}
              disabled={loading || !statement}
              className="bg-teal-700 hover:bg-teal-800 text-white font-semibold gap-1.5"
            >
              <Printer className="size-4" />
              Imprimir {format === "tirilla" ? "Tirilla" : "Carta"}
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Impresión con PrintArea */}
      {statement && (
        <PrintArea
          width={format === "tirilla" ? ticketWidthMm : undefined}
          className={format === "carta" ? "carta" : undefined}
        >
          {format === "tirilla" ? (
            <div className="space-y-1">
              <p className="t-center t-bold t-big">{businessName}</p>
              {nit && <p className="t-center">{nit}</p>}
              <hr />
              <p className="t-center t-bold t-big">ESTADO DE CUENTA</p>
              <p className="t-row">
                <span>Cliente</span>
                <span>{customerName}</span>
              </p>
              {customerDoc && (
                <p className="t-row">
                  <span>Doc</span>
                  <span>{customerDoc}</span>
                </p>
              )}
              <p className="t-row">
                <span>Período</span>
                <span>{formatDateDMY(from)} al {formatDateDMY(to)}</span>
              </p>
              <hr />
              <p className="t-row t-bold">
                <span>Saldo inicial</span>
                <span>{formatCOP(Number(statement.opening_balance))}</span>
              </p>
              <hr />
              {statement.entries.map((e, i) => (
                <div key={i} className="t-item">
                  <p className="t-row">
                    <span>{formatDateDMY(e.date)} · {e.type}</span>
                    <span>{Number(e.debit) > 0 ? `+${formatCOP(Number(e.debit))}` : `-${formatCOP(Number(e.credit))}`}</span>
                  </p>
                  <p className="t-row t-small text-slate-500">
                    <span>{e.reference}</span>
                    <span>Saldo: {formatCOP(Number(e.balance))}</span>
                  </p>
                </div>
              ))}
              <hr />
              <p className="t-row t-bold t-big">
                <span>Saldo final</span>
                <span>{formatCOP(Number(statement.closing_balance))}</span>
              </p>
            </div>
          ) : (
            <div className="p-4 space-y-4">
              <div className="flex justify-between items-start border-b pb-4">
                <div>
                  <h1 className="text-xl font-bold">{businessName}</h1>
                  {nit && <p className="text-sm text-slate-500">{nit}</p>}
                </div>
                <div className="text-right">
                  <h2 className="text-lg font-bold uppercase text-slate-800">Estado de Cuenta</h2>
                  <p className="text-xs text-slate-500">
                    Período: {formatDateDMY(from)} al {formatDateDMY(to)}
                  </p>
                </div>
              </div>

              <div className="border rounded-lg p-3 bg-slate-50 flex justify-between text-sm">
                <div>
                  <span className="font-semibold text-slate-700">Cliente: </span>
                  <span>{customerName}</span>
                  {customerDoc && <span className="ml-2 text-slate-500">({customerDoc})</span>}
                </div>
                <div className="space-x-4">
                  <span>
                    <strong>Saldo inicial:</strong> {formatCOP(Number(statement.opening_balance))}
                  </span>
                  <span>
                    <strong>Saldo final:</strong> {formatCOP(Number(statement.closing_balance))}
                  </span>
                </div>
              </div>

              <table className="w-full text-left text-xs border-collapse">
                <thead>
                  <tr className="border-b bg-slate-100 font-bold">
                    <th className="py-2 px-2">Fecha</th>
                    <th className="py-2 px-2">Tipo</th>
                    <th className="py-2 px-2">Referencia</th>
                    <th className="py-2 px-2 text-right">Cargo</th>
                    <th className="py-2 px-2 text-right">Abono</th>
                    <th className="py-2 px-2 text-right">Saldo</th>
                  </tr>
                </thead>
                <tbody>
                  {statement.entries.map((e, i) => (
                    <tr key={i} className="border-b">
                      <td className="py-2 px-2">{formatDateDMY(e.date)}</td>
                      <td className="py-2 px-2 font-semibold">{e.type}</td>
                      <td className="py-2 px-2">{e.reference}</td>
                      <td className="py-2 px-2 text-right">
                        {Number(e.debit) > 0 ? formatCOP(Number(e.debit)) : "-"}
                      </td>
                      <td className="py-2 px-2 text-right">
                        {Number(e.credit) > 0 ? formatCOP(Number(e.credit)) : "-"}
                      </td>
                      <td className="py-2 px-2 text-right font-bold">
                        {formatCOP(Number(e.balance))}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          )}
        </PrintArea>
      )}
    </>
  );
}
