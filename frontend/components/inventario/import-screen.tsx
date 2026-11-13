"use client";

import { useRef, useState } from "react";
import Link from "next/link";
import {
  AlertCircle,
  AlertTriangle,
  ArrowLeft,
  CheckCircle2,
  Download,
  FileSpreadsheet,
  FileUp,
  RefreshCw,
  Upload,
  XCircle,
} from "lucide-react";

import {
  api,
  type AnalyzeResponse,
} from "@/lib/api";
import { useCurrentBranch } from "@/lib/stores/session";
import { formatCOP, formatQty } from "@/lib/format";
import { cn } from "@/lib/utils";

type Step = "UPLOAD" | "PREVIEW" | "COMMITTING" | "COMPLETED";
type FilterStatus = "TODOS" | "OK" | "ADVERTENCIA" | "ERROR";

export function ImportScreen() {
  const branch = useCurrentBranch();
  const fileInputRef = useRef<HTMLInputElement>(null);

  const [step, setStep] = useState<Step>("UPLOAD");
  const [file, setFile] = useState<File | null>(null);
  const [analyzing, setAnalyzing] = useState(false);
  const [errorMsg, setErrorMsg] = useState<string | null>(null);

  // Analyze result
  const [analysis, setAnalysis] = useState<AnalyzeResponse | null>(null);
  const [filterStatus, setFilterStatus] = useState<FilterStatus>("TODOS");

  // Commit progress
  const [committedCount, setCommittedCount] = useState(0);
  const [totalToCommit, setTotalToCommit] = useState(0);

  function handleFileSelect(selectedFile: File) {
    if (!selectedFile.name.toLowerCase().endsWith(".csv")) {
      setErrorMsg("Solo se admiten archivos en formato CSV (.csv)");
      return;
    }
    if (selectedFile.size > 2 * 1024 * 1024) {
      setErrorMsg("El archivo excede el tamaño máximo permitido de 2 MB");
      return;
    }
    setErrorMsg(null);
    setFile(selectedFile);
  }

  async function handleAnalyze() {
    if (!file) return;
    setAnalyzing(true);
    setErrorMsg(null);
    try {
      const res = await api.products.importAnalyze(branch.id, file);
      setAnalysis(res);
      setStep("PREVIEW");
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : "Error al analizar archivo CSV";
      setErrorMsg(msg);
    } finally {
      setAnalyzing(false);
    }
  }

  async function handleCommit() {
    if (!analysis) return;
    setStep("COMMITTING");
    setErrorMsg(null);
    const total = analysis.rows.length;
    setTotalToCommit(total);
    setCommittedCount(0);

    const batchSize = 200;
    let offset = 0;

    try {
      while (offset < total) {
        const res = await api.products.importCommit(analysis.session_id, offset, batchSize);
        setCommittedCount(res.committed_rows);
        if (res.done) break;
        offset += batchSize;
      }
      setStep("COMPLETED");
    } catch (err: unknown) {
      const msg = err instanceof Error ? err.message : "Error al procesar la importación";
      setErrorMsg(msg);
      setStep("PREVIEW");
    }
  }

  const filteredRows = (analysis?.rows ?? []).filter((r) => {
    if (filterStatus === "TODOS") return true;
    return r.status === filterStatus;
  });

  return (
    <div className="space-y-6 p-6 lg:p-10 max-w-6xl mx-auto">
      {/* Header */}
      <div className="flex flex-wrap items-center justify-between gap-4">
        <div className="flex items-center gap-3">
          <Link
            href="/inventario"
            className="flex size-10 items-center justify-center rounded-full border border-slate-200 bg-white hover:bg-slate-50 text-slate-600 transition-colors"
          >
            <ArrowLeft className="size-4" />
          </Link>
          <div>
            <h1 className="text-2xl font-bold tracking-tight">Importación masiva de productos</h1>
            <p className="text-sm text-slate-500">
              Carga tu catálogo desde Excel o CSV para la sucursal {branch.name}
            </p>
          </div>
        </div>

        <a
          href="/plantilla-productos.csv"
          download="plantilla-productos.csv"
          className="flex h-10 items-center gap-2 rounded-full border border-slate-200 bg-white px-4 text-sm font-medium text-slate-700 hover:bg-slate-50 transition-colors shadow-sm"
        >
          <Download className="size-4" /> Descargar plantilla CSV
        </a>
      </div>

      {errorMsg && (
        <div className="flex items-center gap-2 rounded-2xl border border-rose-200 bg-rose-50 p-4 text-sm font-medium text-rose-700">
          <AlertCircle className="size-5 shrink-0" /> {errorMsg}
        </div>
      )}

      {/* Step 1: Subir Archivo */}
      {step === "UPLOAD" && (
        <div className="space-y-6">
          <div
            onDragOver={(e) => e.preventDefault()}
            onDrop={(e) => {
              e.preventDefault();
              if (e.dataTransfer.files?.[0]) handleFileSelect(e.dataTransfer.files[0]);
            }}
            onClick={() => fileInputRef.current?.click()}
            className="flex flex-col items-center justify-center border-2 border-dashed border-slate-300 hover:border-emerald-500 rounded-3xl p-12 bg-white/60 hover:bg-emerald-50/20 cursor-pointer transition-colors text-center"
          >
            <input
              type="file"
              ref={fileInputRef}
              accept=".csv"
              onChange={(e) => {
                if (e.target.files?.[0]) handleFileSelect(e.target.files[0]);
              }}
              className="hidden"
            />
            <div className="size-16 rounded-2xl bg-emerald-100/70 text-emerald-700 flex items-center justify-center mb-4">
              <FileUp className="size-8" />
            </div>
            <h3 className="text-lg font-bold text-slate-900">
              {file ? file.name : "Arrastra tu archivo CSV aquí"}
            </h3>
            <p className="mt-1 text-sm text-slate-500 max-w-sm">
              {file
                ? `${(file.size / 1024).toFixed(1)} KB seleccionados. Haz clic para cambiar.`
                : "o haz clic para explorar los archivos de tu computadora (máximo 2 MB o 5.000 filas)"}
            </p>
          </div>

          <div className="flex justify-end gap-3">
            <Link
              href="/inventario"
              className="px-5 py-2.5 rounded-full border border-slate-200 text-sm font-medium hover:bg-slate-50"
            >
              Cancelar
            </Link>
            <button
              type="button"
              disabled={!file || analyzing}
              onClick={handleAnalyze}
              className="flex items-center gap-2 px-6 py-2.5 rounded-full bg-emerald-600 text-white font-semibold text-sm shadow hover:bg-emerald-700 disabled:opacity-50 transition-colors"
            >
              {analyzing ? (
                <>
                  <RefreshCw className="size-4 animate-spin" /> Analizando archivo...
                </>
              ) : (
                <>
                  <Upload className="size-4" /> Continuar a validación
                </>
              )}
            </button>
          </div>
        </div>
      )}

      {/* Step 2: Revisión y Previsualización */}
      {step === "PREVIEW" && analysis && (
        <div className="space-y-6">
          {/* Tarjetas de Resumen */}
          <div className="grid grid-cols-2 md:grid-cols-5 gap-3">
            <div className="p-4 rounded-2xl border border-slate-200 bg-white">
              <span className="text-xs font-semibold text-slate-500 uppercase">Total Filas</span>
              <p className="text-2xl font-bold text-slate-900 mt-1">{analysis.rows.length}</p>
            </div>
            <div className="p-4 rounded-2xl border border-emerald-200 bg-emerald-50/50">
              <span className="text-xs font-semibold text-emerald-700 uppercase">Válidas (OK)</span>
              <p className="text-2xl font-bold text-emerald-700 mt-1">{analysis.summary.ok}</p>
            </div>
            <div className="p-4 rounded-2xl border border-amber-200 bg-amber-50/50">
              <span className="text-xs font-semibold text-amber-700 uppercase">Advertencias</span>
              <p className="text-2xl font-bold text-amber-700 mt-1">{analysis.summary.warnings}</p>
            </div>
            <div className="p-4 rounded-2xl border border-rose-200 bg-rose-50/50">
              <span className="text-xs font-semibold text-rose-700 uppercase">Errores</span>
              <p className="text-2xl font-bold text-rose-700 mt-1">{analysis.summary.errors}</p>
            </div>
            <div className="p-4 rounded-2xl border border-blue-200 bg-blue-50/50">
              <span className="text-xs font-semibold text-blue-700 uppercase">Nuevos / Act.</span>
              <p className="text-2xl font-bold text-blue-700 mt-1">
                {analysis.summary.create} / {analysis.summary.update}
              </p>
            </div>
          </div>

          {analysis.summary.errors > 0 ? (
            <div className="p-4 rounded-2xl border border-rose-300 bg-rose-50 text-rose-900 flex items-start gap-3">
              <XCircle className="size-5 text-rose-600 mt-0.5 shrink-0" />
              <div>
                <p className="font-semibold text-sm">
                  El archivo contiene {analysis.summary.errors} {analysis.summary.errors === 1 ? "error" : "errores"} que impiden la importación.
                </p>
                <p className="text-xs text-rose-700 mt-0.5">
                  Corrige los datos marcados en rojo en tu archivo CSV y vuelve a subirlo.
                </p>
              </div>
            </div>
          ) : (
            <div className="p-4 rounded-2xl border border-emerald-300 bg-emerald-50 text-emerald-900 flex items-start gap-3">
              <CheckCircle2 className="size-5 text-emerald-600 mt-0.5 shrink-0" />
              <div>
                <p className="font-semibold text-sm">Validación completada sin errores de bloqueo.</p>
                <p className="text-xs text-emerald-700 mt-0.5">
                  Todo está listo para cargar los {analysis.rows.length} productos en el catálogo de {branch.name}.
                </p>
              </div>
            </div>
          )}

          {/* Filtros de la tabla */}
          <div className="flex items-center justify-between gap-4 border-b border-slate-200 pb-3">
            <div className="flex gap-2">
              {(["TODOS", "OK", "ADVERTENCIA", "ERROR"] as FilterStatus[]).map((st) => (
                <button
                  key={st}
                  type="button"
                  onClick={() => setFilterStatus(st)}
                  className={cn(
                    "px-3.5 py-1.5 rounded-full text-xs font-semibold transition-colors",
                    filterStatus === st
                      ? "bg-slate-900 text-white"
                      : "bg-slate-100 text-slate-600 hover:bg-slate-200"
                  )}
                >
                  {st === "TODOS"
                    ? `Todos (${analysis.rows.length})`
                    : st === "OK"
                      ? `OK (${analysis.summary.ok})`
                      : st === "ADVERTENCIA"
                        ? `Advertencias (${analysis.summary.warnings})`
                        : `Errores (${analysis.summary.errors})`}
                </button>
              ))}
            </div>

            <button
              type="button"
              onClick={() => {
                setAnalysis(null);
                setStep("UPLOAD");
              }}
              className="text-xs text-slate-500 hover:text-slate-800 underline font-medium"
            >
              Subir otro archivo
            </button>
          </div>

          {/* Tabla de Resultados */}
          <div className="overflow-x-auto rounded-2xl border border-slate-200 bg-white shadow-sm max-h-[500px]">
            <table className="w-full text-left text-xs">
              <thead className="bg-slate-50 text-slate-600 sticky top-0 border-b border-slate-200">
                <tr>
                  <th className="p-3 font-semibold">Fila</th>
                  <th className="p-3 font-semibold">Estado</th>
                  <th className="p-3 font-semibold">Acción</th>
                  <th className="p-3 font-semibold">Nombre</th>
                  <th className="p-3 font-semibold">SKU / Código</th>
                  <th className="p-3 font-semibold">Categoría</th>
                  <th className="p-3 font-semibold text-right">Precio</th>
                  <th className="p-3 font-semibold text-right">Costo</th>
                  <th className="p-3 font-semibold text-right">Cant. Inicial</th>
                  <th className="p-3 font-semibold">Observaciones</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100">
                {filteredRows.map((row) => (
                  <tr
                    key={row.line}
                    className={cn(
                      "hover:bg-slate-50/60 transition-colors",
                      row.status === "ERROR" && "bg-rose-50/30",
                      row.status === "ADVERTENCIA" && "bg-amber-50/20"
                    )}
                  >
                    <td className="p-3 font-mono text-slate-400">{row.line}</td>
                    <td className="p-3">
                      <span
                        className={cn(
                          "inline-flex items-center gap-1 rounded-full px-2 py-0.5 font-semibold text-[11px]",
                          row.status === "OK" && "bg-emerald-100 text-emerald-800",
                          row.status === "ADVERTENCIA" && "bg-amber-100 text-amber-800",
                          row.status === "ERROR" && "bg-rose-100 text-rose-800"
                        )}
                      >
                        {row.status === "OK" ? (
                          <CheckCircle2 className="size-3" />
                        ) : row.status === "ADVERTENCIA" ? (
                          <AlertTriangle className="size-3" />
                        ) : (
                          <XCircle className="size-3" />
                        )}
                        {row.status}
                      </span>
                    </td>
                    <td className="p-3">
                      <span
                        className={cn(
                          "rounded-md px-1.5 py-0.5 font-bold text-[10px]",
                          row.action === "CREAR"
                            ? "bg-blue-50 text-blue-700 border border-blue-200"
                            : "bg-purple-50 text-purple-700 border border-purple-200"
                        )}
                      >
                        {row.action}
                      </span>
                    </td>
                    <td className="p-3 font-medium text-slate-800 max-w-[200px] truncate">
                      {row.data.nombre}
                      {row.data.sku_padre && (
                        <span className="block text-[10px] text-slate-400">
                          Variante de: {row.data.sku_padre}
                        </span>
                      )}
                    </td>
                    <td className="p-3 font-mono text-slate-600">
                      <div>{row.data.sku || "—"}</div>
                      {row.data.codigo_barras && (
                        <div className="text-[10px] text-slate-400">{row.data.codigo_barras}</div>
                      )}
                    </td>
                    <td className="p-3 text-slate-600">{row.data.categoria || "—"}</td>
                    <td className="p-3 text-right font-mono font-medium text-slate-900">
                      {row.data.precio ? formatCOP(Number(row.data.precio)) : "—"}
                    </td>
                    <td className="p-3 text-right font-mono text-slate-500">
                      {row.data.costo ? formatCOP(Number(row.data.costo)) : "—"}
                    </td>
                    <td className="p-3 text-right font-mono text-slate-700">
                      {row.data.cantidad_inicial ? formatQty(row.data.cantidad_inicial) : "0"}
                    </td>
                    <td className="p-3 text-slate-600 max-w-xs">
                      {row.messages.length > 0 ? (
                        <ul className="list-disc list-inside space-y-0.5">
                          {row.messages.map((m, i) => (
                            <li
                              key={i}
                              className={cn(
                                row.status === "ERROR" ? "text-rose-600" : "text-amber-600"
                              )}
                            >
                              {m}
                            </li>
                          ))}
                        </ul>
                      ) : (
                        "—"
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>

          {/* Botones de acción */}
          <div className="flex justify-end gap-3 pt-2">
            <button
              type="button"
              onClick={() => {
                setAnalysis(null);
                setStep("UPLOAD");
              }}
              className="px-5 py-2.5 rounded-full border border-slate-200 text-sm font-medium hover:bg-slate-50"
            >
              Cancelar
            </button>
            <button
              type="button"
              disabled={analysis.summary.errors > 0}
              onClick={handleCommit}
              className="flex items-center gap-2 px-6 py-2.5 rounded-full bg-emerald-600 text-white font-semibold text-sm shadow hover:bg-emerald-700 disabled:opacity-50 transition-colors"
            >
              <FileSpreadsheet className="size-4" /> Importar {analysis.rows.length} productos
            </button>
          </div>
        </div>
      )}

      {/* Step 3: Progreso de Confirmación */}
      {step === "COMMITTING" && (
        <div className="rounded-3xl border border-slate-200 bg-white p-12 text-center max-w-xl mx-auto space-y-6 shadow-sm">
          <div className="size-16 rounded-2xl bg-emerald-50 text-emerald-600 flex items-center justify-center mx-auto animate-pulse">
            <RefreshCw className="size-8 animate-spin" />
          </div>

          <div>
            <h3 className="text-xl font-bold text-slate-900">Importando productos...</h3>
            <p className="text-sm text-slate-500 mt-1">
              Procesando fila {committedCount} de {totalToCommit}
            </p>
          </div>

          <div className="w-full bg-slate-100 h-3 rounded-full overflow-hidden">
            <div
              className="bg-emerald-500 h-full transition-all duration-300 rounded-full"
              style={{
                width: `${totalToCommit > 0 ? (committedCount / totalToCommit) * 100 : 0}%`,
              }}
            />
          </div>
        </div>
      )}

      {/* Step 4: Completado */}
      {step === "COMPLETED" && (
        <div className="rounded-3xl border border-emerald-200 bg-emerald-50/40 p-12 text-center max-w-xl mx-auto space-y-6 shadow-sm">
          <div className="size-16 rounded-2xl bg-emerald-100 text-emerald-700 flex items-center justify-center mx-auto">
            <CheckCircle2 className="size-8" />
          </div>

          <div>
            <h3 className="text-xl font-bold text-slate-900">¡Importación completada con éxito!</h3>
            <p className="text-sm text-slate-600 mt-1">
              Se han creado y actualizado {committedCount} productos en el inventario.
            </p>
          </div>

          <div className="flex justify-center gap-3 pt-2">
            <Link
              href="/inventario"
              className="px-6 py-2.5 rounded-full bg-emerald-600 text-white font-semibold text-sm shadow hover:bg-emerald-700 transition-colors"
            >
              Ver inventario
            </Link>
          </div>
        </div>
      )}
    </div>
  );
}
