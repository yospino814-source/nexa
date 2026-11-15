"use client";

import { useState } from "react";
import Link from "next/link";
import { Check, Copy, Download, Loader2, MessageCircle, Printer, Store } from "lucide-react";

import { api, friendlyError } from "@/lib/api";
import { saveBlob } from "@/lib/api/client";
import { useApi } from "@/lib/api/use-api";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { PrintArea } from "@/components/nexa/print-area";

/** El SVG lo genera nuestro backend; se muestra como imagen (sin inyectar HTML). */
function svgSrc(svg: string): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}

/**
 * QR de cada tienda: abre el chat de pedidos por WhatsApp con un mensaje ya
 * escrito. Se imprime como cartel de mostrador (carta) o se descarga en SVG
 * para volantes y redes.
 */
export function WhatsappQrScreen() {
  const currentBranch = useCurrentBranch();
  const branches = useSession((s) => s.branches);
  const [branchId, setBranchId] = useState<string>(currentBranch.id);
  const [draft, setDraft] = useState("Hola, quiero hacer un pedido");
  const [text, setText] = useState("Hola, quiero hacer un pedido");
  const [copied, setCopied] = useState(false);

  const { data: qr, loading, error } = useApi(() => api.whatsapp.storeQr(branchId, text), `${branchId}|${text}`);
  const branchName = branches.find((b) => b.id === branchId)?.name ?? "";

  async function copyLink() {
    if (!qr) return;
    await navigator.clipboard.writeText(qr.link);
    setCopied(true);
    setTimeout(() => setCopied(false), 2000);
  }

  function downloadSvg() {
    if (!qr) return;
    const slug = branchName.toLowerCase().normalize("NFD").replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "tienda";
    saveBlob(new Blob([qr.svg], { type: "image/svg+xml" }), `qr-whatsapp-${slug}.svg`);
  }

  return (
    <div className="space-y-6 p-6 lg:p-10">
      <div className="flex flex-wrap items-end justify-between gap-4">
        <div>
          <h1 className="text-3xl font-bold tracking-tight">QR de WhatsApp de la tienda</h1>
          <p className="mt-1 max-w-2xl text-slate-500">
            El cliente lo escanea con la cámara y se abre el chat de pedidos de esta sede, con el mensaje ya escrito.
            Imprimilo para el mostrador o descargalo para volantes y redes.
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

      <div className="grid gap-6 lg:grid-cols-[minmax(0,1fr)_380px]">
        <section aria-label="Vista previa del cartel" className="flex justify-center rounded-3xl bg-slate-100 p-6 lg:p-10">
          {loading && !qr ? (
            <div className="flex h-96 items-center justify-center text-slate-500">
              <Loader2 className="size-6 animate-spin" aria-hidden />
              <span className="sr-only">Generando el QR</span>
            </div>
          ) : error ? (
            <div className="flex max-w-md flex-col items-center gap-3 self-center text-center">
              <MessageCircle className="size-10 text-slate-400" aria-hidden />
              <p className="font-semibold text-slate-800">{friendlyError(error)}</p>
              <p className="text-sm text-slate-500">
                Configurá el número de WhatsApp del bot de esta sede y volvé acá.
              </p>
            </div>
          ) : qr ? (
            <Poster name={qr.business_name} phone={qr.display_phone} svg={qr.svg} branch={branchName} />
          ) : null}
        </section>

        <aside className="space-y-5">
          <form
            className="space-y-3 rounded-2xl border border-slate-200 bg-white p-5"
            onSubmit={(e) => {
              e.preventDefault();
              setText(draft);
            }}
          >
            <label htmlFor="qr-text" className="block text-sm font-semibold text-slate-800">
              Mensaje que llega escrito
            </label>
            <textarea
              id="qr-text"
              value={draft}
              onChange={(e) => setDraft(e.target.value)}
              maxLength={200}
              rows={3}
              className="w-full rounded-xl border border-slate-200 p-3 text-sm outline-none focus:border-indigo-500 focus:ring-4 focus:ring-indigo-100"
            />
            <p className="text-xs text-slate-500">
              Ayuda a que Nexi sepa de dónde viene el cliente, por ejemplo: «Hola, vi el QR del mostrador».
            </p>
            <button
              type="submit"
              disabled={draft.trim() === text.trim()}
              className="h-11 w-full rounded-xl bg-slate-900 text-sm font-semibold text-white disabled:opacity-40"
            >
              Actualizar el QR
            </button>
          </form>

          <div className="space-y-2 rounded-2xl border border-slate-200 bg-white p-5">
            <button
              type="button"
              onClick={() => window.print()}
              disabled={!qr}
              className="flex h-12 w-full items-center justify-center gap-2 rounded-xl bg-indigo-600 text-sm font-semibold text-white disabled:opacity-40"
            >
              <Printer className="size-4" aria-hidden /> Imprimir cartel (carta)
            </button>
            <button
              type="button"
              onClick={downloadSvg}
              disabled={!qr}
              className="flex h-11 w-full items-center justify-center gap-2 rounded-xl border border-slate-200 text-sm font-semibold text-slate-800 disabled:opacity-40"
            >
              <Download className="size-4" aria-hidden /> Descargar QR (SVG)
            </button>
            <button
              type="button"
              onClick={copyLink}
              disabled={!qr}
              className="flex h-11 w-full items-center justify-center gap-2 rounded-xl border border-slate-200 text-sm font-semibold text-slate-800 disabled:opacity-40"
            >
              {copied ? <Check className="size-4 text-emerald-600" aria-hidden /> : <Copy className="size-4" aria-hidden />}
              {copied ? "Enlace copiado" : "Copiar enlace para redes"}
            </button>
            {qr && <p className="break-all pt-1 text-xs text-slate-500">{qr.link}</p>}
          </div>

          <p className="text-sm text-slate-500">
            ¿Cambió el número? Se actualiza en la{" "}
            <Link href="/configuracion" className="font-semibold text-indigo-600 hover:underline">
              configuración del bot
            </Link>{" "}
            y el QR nuevo sale solo. Los carteles viejos dejan de servir: reimprimilos.
          </p>
        </aside>
      </div>

      {qr && (
        <PrintArea className="carta">
          <Poster name={qr.business_name} phone={qr.display_phone} svg={qr.svg} branch={branchName} print />
        </PrintArea>
      )}
    </div>
  );
}

function Poster({ name, phone, svg, branch, print = false }: { name: string; phone: string; svg: string; branch: string; print?: boolean }) {
  return (
    <div
      className={
        print
          ? "mx-auto flex flex-col items-center gap-6 text-center"
          : "flex w-full max-w-md flex-col items-center gap-5 rounded-3xl bg-white p-8 text-center shadow-sm"
      }
      style={print ? { width: "170mm", padding: "12mm 0" } : undefined}
    >
      <p className={print ? "text-[18pt] font-bold uppercase tracking-[0.12em] text-indigo-700" : "text-xs font-bold uppercase tracking-[0.14em] text-indigo-700"}>
        Pedí por WhatsApp
      </p>
      <h2 className={print ? "text-[34pt] font-extrabold leading-tight" : "text-3xl font-extrabold leading-tight"}>{name}</h2>
      {/* eslint-disable-next-line @next/next/no-img-element */}
      <img
        src={svgSrc(svg)}
        alt={`Código QR para escribirle a ${name} por WhatsApp`}
        width={print ? 420 : 260}
        height={print ? 420 : 260}
        style={print ? { width: "110mm", height: "110mm" } : undefined}
      />
      <p className={print ? "text-[16pt] text-slate-700" : "text-base text-slate-700"}>
        Escaneá con la cámara del celular y escribinos tu pedido.
        <br />
        Te lo llevamos a la casa.
      </p>
      <p className={print ? "text-[20pt] font-bold" : "text-lg font-bold"}>{phone}</p>
      {branch && <p className={print ? "text-[12pt] text-slate-500" : "text-sm text-slate-500"}>{branch}</p>}
    </div>
  );
}
