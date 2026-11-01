"use client";

import { useEffect, useMemo, useState } from "react";
import { ArrowLeftRight, Loader2 } from "lucide-react";

import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { api, friendlyError, type BranchAccess, type LocationRow, type StockLevel } from "@/lib/api";
import { formatQty } from "@/lib/format";

/** Traslado de unidades a otra ubicación o sucursal del negocio (F2.8). */
export function TransferDialog({
  item,
  from,
  destinations,
  onClose,
  onSaved,
}: {
  item: StockLevel | null;
  from: BranchAccess;
  /** Sucursales donde el usuario administra inventario (sin la actual). */
  destinations: BranchAccess[];
  onClose: () => void;
  onSaved: (message: string) => void;
}) {
  const allBranches = useMemo(() => [from, ...destinations], [from, destinations]);
  const [branchLocations, setBranchLocations] = useState<Record<string, LocationRow[]>>({});
  const [loadingLocations, setLoadingLocations] = useState(true);

  const [fromLocId, setFromLocId] = useState("");
  const [toLocId, setToLocId] = useState("");
  const [qty, setQty] = useState("");
  const [notes, setNotes] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let isSubscribed = true;

    Promise.all(
      allBranches.map((b) =>
        api.locations.list(b.id).then((locs) => ({ bId: b.id, locs })).catch(() => ({ bId: b.id, locs: [] }))
      )
    )
      .then((results) => {
        if (!isSubscribed) return;
        const map: Record<string, LocationRow[]> = {};
        for (const r of results) {
          map[r.bId] = r.locs;
        }
        setBranchLocations(map);

        const fromLocs = map[from.id] ?? [];
        const defFrom = fromLocs.find((l) => l.is_default && l.active)?.id ?? fromLocs[0]?.id ?? "";
        setFromLocId(defFrom);

        // Buscar otra ubicación en la misma sucursal o en otra
        const otherLoc =
          fromLocs.find((l) => l.id !== defFrom && l.active)?.id ??
          (destinations[0]
            ? (map[destinations[0].id]?.find((l) => l.is_default && l.active)?.id ??
              map[destinations[0].id]?.[0]?.id)
            : "");
        setToLocId(otherLoc ?? "");
        setLoadingLocations(false);
      });

    return () => {
      isSubscribed = false;
    };
  }, [allBranches, from.id, destinations]);

  const [lastItem, setLastItem] = useState(item?.product_id);
  if (item?.product_id !== lastItem) {
    setLastItem(item?.product_id);
    setQty("");
    setNotes("");
    setError(null);
  }

  if (!item) return <Dialog open={false} />;
  const n = Number(qty.replace(",", "."));
  const available = Number(item.available);
  const sameLocation = fromLocId && toLocId && fromLocId === toLocId;
  const valid = n > 0 && n <= available && !!fromLocId && !!toLocId && !sameLocation;

  // Encontrar nombres para mensaje
  let toLocName = "destino";
  for (const b of allBranches) {
    const found = branchLocations[b.id]?.find((l) => l.id === toLocId);
    if (found) {
      toLocName = `${found.name} (${b.name})`;
      break;
    }
  }

  async function save() {
    if (!item) return;
    setBusy(true);
    setError(null);
    try {
      await api.inventory.transfer({
        fromLocationId: fromLocId,
        toLocationId: toLocId,
        productId: item.product_id,
        qty: String(n),
        cause: notes.trim() || `Traslado a ${toLocName}`,
      });
      onSaved(`${formatQty(n)} × ${item.name} trasladadas a ${toLocName}`);
      onClose();
    } catch (e) {
      setError(friendlyError(e));
    } finally {
      setBusy(false);
    }
  }

  const fromBranchLocations = (branchLocations[from.id] ?? []).filter((l) => l.active);

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="gap-0 rounded-3xl p-6 sm:max-w-md">
        <div className="flex items-start gap-3">
          <span className="flex size-11 shrink-0 items-center justify-center rounded-2xl bg-sky-50 text-sky-600">
            <ArrowLeftRight className="size-5" />
          </span>
          <div>
            <DialogTitle className="text-lg font-bold">Trasladar existencias</DialogTitle>
            <DialogDescription className="text-sm text-slate-500">
              {item.name} · disponible total en {from.name}: <b>{formatQty(available)}</b>
            </DialogDescription>
          </div>
        </div>

        {loadingLocations ? (
          <div className="flex items-center justify-center py-10 text-slate-400">
            <Loader2 className="size-6 animate-spin" />
          </div>
        ) : (
          <div className="mt-5 space-y-4">
            <div>
              <label className="block text-xs font-bold uppercase tracking-wider text-slate-500">
                Ubicación de origen ({from.name})
              </label>
              <select
                value={fromLocId}
                onChange={(e) => setFromLocId(e.target.value)}
                className="mt-1.5 block w-full rounded-2xl border border-slate-200 bg-white px-3.5 py-2.5 text-sm font-semibold text-slate-900 outline-none focus:border-brand focus:ring-1 focus:ring-brand"
              >
                {fromBranchLocations.map((l) => (
                  <option key={l.id} value={l.id}>
                    {l.name} {l.is_default ? "(Principal)" : ""}
                  </option>
                ))}
              </select>
            </div>

            <div>
              <label className="block text-xs font-bold uppercase tracking-wider text-slate-500">
                Ubicación de destino (por sucursal)
              </label>
              <select
                value={toLocId}
                onChange={(e) => setToLocId(e.target.value)}
                className="mt-1.5 block w-full rounded-2xl border border-slate-200 bg-white px-3.5 py-2.5 text-sm font-semibold text-slate-900 outline-none focus:border-brand focus:ring-1 focus:ring-brand"
              >
                {allBranches.map((b) => {
                  const locs = (branchLocations[b.id] ?? []).filter((l) => l.active);
                  if (locs.length === 0) return null;
                  return (
                    <optgroup key={b.id} label={`Sucursal: ${b.name}`}>
                      {locs.map((l) => (
                        <option key={l.id} value={l.id}>
                          {l.name} {l.is_default ? "(Principal)" : ""}
                        </option>
                      ))}
                    </optgroup>
                  );
                })}
              </select>
              {sameLocation && (
                <p className="mt-1 text-xs text-rose-600">
                  La bodega de origen y destino deben ser distintas.
                </p>
              )}
            </div>

            <div>
              <label htmlFor="tqty" className="block text-xs font-bold uppercase tracking-wider text-slate-500">
                Unidades a trasladar
              </label>
              <input
                id="tqty"
                autoFocus
                inputMode="decimal"
                value={qty}
                onChange={(e) => setQty(e.target.value.replace(/[^\d,.]/g, ""))}
                placeholder="0"
                className="mt-1.5 h-12 w-full rounded-2xl border border-slate-200 text-center text-xl font-bold tabular-nums outline-none focus:ring-2 focus:ring-brand/40"
              />
              {n > available && <p className="mt-1 text-xs text-rose-600">Supera lo disponible.</p>}
            </div>

            <div>
              <label className="block text-xs font-bold uppercase tracking-wider text-slate-500">
                Nota u observación (opcional)
              </label>
              <input
                value={notes}
                onChange={(e) => setNotes(e.target.value)}
                placeholder="Ej. Quién lo lleva, remisión, motivo..."
                className="mt-1.5 h-11 w-full rounded-xl border border-slate-200 px-3 text-sm outline-none focus:ring-2 focus:ring-brand/40"
              />
            </div>
          </div>
        )}

        {error && <p className="mt-4 rounded-xl bg-rose-50 p-3 text-sm text-rose-600">{error}</p>}

        <div className="mt-6 flex justify-end gap-3 border-t border-slate-100 pt-4">
          <button
            onClick={onClose}
            className="h-11 rounded-xl px-4 text-sm font-medium text-slate-600 hover:bg-slate-50"
          >
            Cancelar
          </button>
          <button
            onClick={save}
            disabled={!valid || busy || loadingLocations}
            className="flex h-11 items-center gap-2 rounded-xl bg-brand px-5 text-sm font-semibold text-white hover:bg-brand-dark disabled:opacity-50"
          >
            {busy && <Loader2 className="size-4 animate-spin" />}
            Trasladar
          </button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
