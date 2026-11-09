"use client";

import { useMemo, useRef, useState } from "react";
import {
  Banknote,
  CheckCircle2,
  DollarSign,
  FileText,
  Loader2,
  Paperclip,
  Plus,
  RefreshCw,
  Search,
  TrendingUp,
  Upload,
  Wallet,
  X,
} from "lucide-react";

import { MoneyInput } from "@/components/caja/money-input";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  api,
  friendlyError,
  USE_MOCKS,
  type CreateExpenseInput,
  type CreatePayablePaymentInput,
  type Expense,
  type ExpenseCategory,
  type ExpenseStatus,
  type ExpenseSummary,
  type PayablePaymentMethod,
  type Supplier,
} from "@/lib/api";
import { useApi } from "@/lib/api/use-api";
import { useCurrentBranch, useSession } from "@/lib/stores/session";
import { formatCOP } from "@/lib/format";
import { cn } from "@/lib/utils";

const STATUS_CONFIG: Record<
  ExpenseStatus,
  { label: string; className: string; dot: string }
> = {
  PENDIENTE: {
    label: "Pendiente",
    className: "border-amber-200 bg-amber-50 text-amber-700",
    dot: "bg-amber-500",
  },
  APROBADO: {
    label: "Aprobado",
    className: "border-blue-200 bg-blue-50 text-blue-700",
    dot: "bg-blue-500",
  },
  PAGADO: {
    label: "Pagado",
    className: "border-emerald-200 bg-emerald-50 text-emerald-700",
    dot: "bg-emerald-500",
  },
  RECHAZADO: {
    label: "Rechazado",
    className: "border-rose-200 bg-rose-50 text-rose-700",
    dot: "bg-rose-500",
  },
  CANCELADO: {
    label: "Cancelado",
    className: "border-slate-200 bg-slate-100 text-slate-600",
    dot: "bg-slate-400",
  },
};

const PAYMENT_METHODS = [
  "Transferencia Bancolombia",
  "Dispersión Bancaria",
  "Nequi Empresa",
  "Débito Automático",
  "Efectivo / Caja Menor",
  "Tarjeta Débito",
  "Bancolombia",
];

function formatExpenseDate(isoDate: string) {
  if (!isoDate) return "-";
  try {
    const [y, m, d] = isoDate.split("-");
    const months = [
      "Ene",
      "Feb",
      "Mar",
      "Abr",
      "May",
      "Jun",
      "Jul",
      "Ago",
      "Sep",
      "Oct",
      "Nov",
      "Dic",
    ];
    const monthName = months[parseInt(m, 10) - 1] || m;
    return `${d} ${monthName} ${y}`;
  } catch {
    return isoDate;
  }
}

export function ExpensesScreen() {
  const currentBranch = useCurrentBranch();
  const branchId = currentBranch?.id;
  const canCreate = useSession((s) => s.can("expenses:create"));
  const canApprove = useSession((s) => s.can("expenses:approve"));
  const canPay = useSession((s) => s.can("purchases:pay"));

  // Filtros
  const [search, setSearch] = useState("");
  const [selectedStatus, setSelectedStatus] = useState<ExpenseStatus | "ALL">("ALL");
  const [selectedCategoryId, setSelectedCategoryId] = useState<string>("ALL");
  const [selectedSupplierId, setSelectedSupplierId] = useState<string>("ALL");
  const [selectedPaymentMethod, setSelectedPaymentMethod] = useState<string>("ALL");
  const [fromDate, setFromDate] = useState("");
  const [toDate, setToDate] = useState("");

  // Modales
  const [isCreateModalOpen, setIsCreateModalOpen] = useState(false);
  const [rejectModalExpense, setRejectModalExpense] = useState<Expense | null>(null);
  const [paymentModalExpense, setPaymentModalExpense] = useState<Expense | null>(null);

  // Formulario nuevo gasto
  const [formCategoryId, setFormCategoryId] = useState<string>("");
  const [formSupplierId, setFormSupplierId] = useState<string>("");
  const [formDescription, setFormDescription] = useState("");
  const [formAmount, setFormAmount] = useState<number>(0);
  const [formTaxAmount, setFormTaxAmount] = useState<number>(0);
  const [formDate, setFormDate] = useState(
    new Date().toISOString().slice(0, 10),
  );
  const [formPaymentMethod, setFormPaymentMethod] = useState(
    "Transferencia Bancolombia",
  );
  const [formAttachmentName, setFormAttachmentName] = useState<string>("");
  const [createBusy, setCreateBusy] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  // Formulario rechazo
  const [rejectReason, setRejectReason] = useState("");
  const [rejectBusy, setRejectBusy] = useState(false);
  const [rejectError, setRejectError] = useState<string | null>(null);

  // Formulario pago de cuenta por pagar vinculada
  const [paymentAmount, setPaymentAmount] = useState<number>(0);
  const [payableMethod, setPayableMethod] = useState<PayablePaymentMethod>("EFECTIVO");
  const [paymentReference, setPaymentReference] = useState("");
  const [paymentBusy, setPaymentBusy] = useState(false);
  const [paymentError, setPaymentError] = useState<string | null>(null);

  // Toast
  const [toast, setToast] = useState<string | null>(null);

  const showToast = (msg: string) => {
    setToast(msg);
    setTimeout(() => setToast(null), 4000);
  };

  // Cargar datos
  const {
    data: categoriesData,
    reload: reloadCategories,
  } = useApi<ExpenseCategory[]>(
    () => api.expenseCategories.list(),
    "expenses:categories",
  );
  const categories = useMemo(() => categoriesData ?? [], [categoriesData]);

  const {
    data: suppliersData,
  } = useApi<Supplier[]>(
    () => api.suppliers.list(),
    "expenses:suppliers",
  );
  const suppliers = useMemo(() => suppliersData ?? [], [suppliersData]);

  const {
    data: expensesData,
    loading: loadingExpenses,
    reload: reloadExpenses,
  } = useApi<Expense[]>(
    () => api.expenses.list({ branch_id: branchId }),
    `expenses:list:${branchId || ""}`,
  );
  const expenses = useMemo(() => expensesData ?? [], [expensesData]);

  const currentMonthStr = useMemo(() => {
    const now = new Date();
    return `${now.getFullYear()}-${String(now.getMonth() + 1).padStart(2, "0")}`;
  }, []);

  const {
    reload: reloadSummary,
  } = useApi<ExpenseSummary>(
    () => api.expenses.summary({ branch_id: branchId, month: currentMonthStr }),
    `expenses:summary:${branchId || ""}:${currentMonthStr}`,
  );

  const reloadAll = () => {
    reloadExpenses();
    reloadSummary();
    reloadCategories();
  };

  // Filtrado de gastos en cliente
  const filteredExpenses = useMemo(() => {
    return expenses.filter((e) => {
      if (selectedStatus !== "ALL" && e.status !== selectedStatus) return false;
      if (selectedCategoryId !== "ALL" && e.category_id !== selectedCategoryId) return false;
      if (selectedSupplierId !== "ALL" && e.supplier_id !== selectedSupplierId) return false;
      if (USE_MOCKS && selectedPaymentMethod !== "ALL" && e.payment_method !== selectedPaymentMethod) return false;
      if (fromDate && e.expense_date < fromDate) return false;
      if (toDate && e.expense_date > toDate) return false;
      if (search.trim()) {
        const q = search.toLowerCase();
        const matchesDoc = e.document_number.toLowerCase().includes(q);
        const matchesDesc = e.description.toLowerCase().includes(q);
        const matchesCat = e.category_name.toLowerCase().includes(q);
        const matchesSupp = e.supplier_name?.toLowerCase().includes(q) ?? false;
        const matchesMethod = e.payment_method?.toLowerCase().includes(q) ?? false;
        if (!matchesDoc && !matchesDesc && !matchesCat && !matchesSupp && !matchesMethod) return false;
      }
      return true;
    });
  }, [expenses, selectedStatus, selectedCategoryId, selectedSupplierId, selectedPaymentMethod, fromDate, toDate, search]);

  // Agrupación por categoría para gráfico de barras horizontal (ordenado de mayor a menor)
  const categoryStats = useMemo(() => {
    const map = new Map<string, { name: string; color: string; total: number }>();
    for (const exp of filteredExpenses) {
      if (exp.status === "RECHAZADO" || exp.status === "CANCELADO") continue;
      const prev = map.get(exp.category_id) || {
        name: exp.category_name,
        color: exp.category_color,
        total: 0,
      };
      prev.total += Number(exp.total);
      map.set(exp.category_id, prev);
    }
    const arr = Array.from(map.values());
    arr.sort((a, b) => b.total - a.total);
    return arr;
  }, [filteredExpenses]);

  // Total de gastos activos en el mes y métricas para las 2 tarjetas resumen
  const totalActiveExpenses = useMemo(() => {
    return categoryStats.reduce((sum, c) => sum + c.total, 0);
  }, [categoryStats]);

  const monthlyBudget = 18_000_000; // Presupuesto mensual estándar COP
  const budgetExecutionPct = monthlyBudget > 0 ? (totalActiveExpenses / monthlyBudget) * 100 : 0;
  const topCategory = categoryStats.length > 0 ? categoryStats[0] : null;
  const topCategoryPct = totalActiveExpenses > 0 && topCategory ? (topCategory.total / totalActiveExpenses) * 100 : 0;

  // Manejar creación
  const handleOpenCreateModal = () => {
    setFormCategoryId(categories[0]?.id || "");
    setFormSupplierId("");
    setFormDescription("");
    setFormAmount(0);
    setFormTaxAmount(0);
    setFormDate(new Date().toISOString().slice(0, 10));
    setFormPaymentMethod("Transferencia Bancolombia");
    setFormAttachmentName("");
    setCreateError(null);
    setIsCreateModalOpen(true);
  };

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (file) {
      setFormAttachmentName(file.name);
    }
  };

  const handleSimulateAttachment = () => {
    setFormAttachmentName("Factura_Soporte.pdf");
  };

  const handleCreateSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!branchId) return;
    if (!formCategoryId) {
      setCreateError("Elegí una categoría.");
      return;
    }
    if (formDescription.trim().length < 3) {
      setCreateError("La descripción debe tener al menos 3 caracteres.");
      return;
    }
    if (formAmount <= 0) {
      setCreateError("El monto debe ser mayor a cero.");
      return;
    }

    try {
      setCreateBusy(true);
      setCreateError(null);
      const input: CreateExpenseInput = {
        id: crypto.randomUUID(),
        branch_id: branchId,
        category_id: formCategoryId,
        supplier_id: formSupplierId || null,
        description: formDescription.trim(),
        amount: formAmount.toFixed(2),
        tax_amount: formTaxAmount > 0 ? formTaxAmount.toFixed(2) : undefined,
        expense_date: formDate,
      };
      if (USE_MOCKS) {
        input.payment_method = formPaymentMethod || "Efectivo / Caja Menor";
        input.attachment_name = formAttachmentName.trim() || null;
      }
      await api.expenses.create(input);
      setIsCreateModalOpen(false);
      showToast("Gasto registrado con éxito.");
      reloadAll();
    } catch (err) {
      setCreateError(friendlyError(err));
    } finally {
      setCreateBusy(false);
    }
  };

  // Manejar aprobación
  const handleApprove = async (expense: Expense) => {
    try {
      await api.expenses.approve(expense.id);
      showToast(`Gasto ${expense.document_number} aprobado. Se generó cuenta por pagar.`);
      reloadAll();
    } catch (err) {
      showToast(friendlyError(err));
    }
  };

  // Manejar rechazo
  const handleOpenReject = (expense: Expense) => {
    setRejectModalExpense(expense);
    setRejectReason("");
    setRejectError(null);
  };

  const handleRejectSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!rejectModalExpense) return;
    if (rejectReason.trim().length < 3) {
      setRejectError("Escribí un motivo de al menos 3 caracteres.");
      return;
    }

    try {
      setRejectBusy(true);
      setRejectError(null);
      await api.expenses.reject(rejectModalExpense.id, {
        reason: rejectReason.trim(),
      });
      showToast(`Gasto ${rejectModalExpense.document_number} rechazado.`);
      setRejectModalExpense(null);
      reloadAll();
    } catch (err) {
      setRejectError(friendlyError(err));
    } finally {
      setRejectBusy(false);
    }
  };

  // Manejar cancelación
  const handleCancel = async (expense: Expense) => {
    if (!confirm(`¿Seguro que deseas cancelar el gasto ${expense.document_number}?`)) return;
    try {
      await api.expenses.cancel(expense.id);
      showToast(`Gasto ${expense.document_number} cancelado.`);
      reloadAll();
    } catch (err) {
      showToast(friendlyError(err));
    }
  };

  // Manejar pago de cuenta por pagar vinculada
  const handleOpenPayment = (expense: Expense) => {
    setPaymentModalExpense(expense);
    const balance = Number(expense.total) - Number(expense.payable_amount_paid || 0);
    setPaymentAmount(Math.max(0, balance));
    setPayableMethod("EFECTIVO");
    setPaymentReference("");
    setPaymentError(null);
  };

  const handlePaymentSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    if (!paymentModalExpense?.payable_id) return;

    if (paymentAmount <= 0) {
      setPaymentError("El monto debe ser mayor a cero.");
      return;
    }
    if ((payableMethod === "TRANSFERENCIA" || payableMethod === "CHEQUE") && !paymentReference.trim()) {
      setPaymentError("La referencia es obligatoria para transferencias o cheques.");
      return;
    }

    try {
      setPaymentBusy(true);
      setPaymentError(null);
      const input: CreatePayablePaymentInput = {
        id: crypto.randomUUID(),
        amount: paymentAmount.toFixed(2),
        method: payableMethod,
        reference: paymentReference.trim() || null,
      };
      await api.payables.createPayment(paymentModalExpense.payable_id, input);
      showToast(`Pago aplicado con éxito al gasto ${paymentModalExpense.document_number}.`);
      setPaymentModalExpense(null);
      reloadAll();
    } catch (err) {
      setPaymentError(friendlyError(err));
    } finally {
      setPaymentBusy(false);
    }
  };

  return (
    <div className="space-y-6 p-6">
      {/* Toast */}
      {toast && (
        <div className="fixed bottom-4 right-4 z-50 rounded-lg border border-emerald-200 bg-emerald-50 px-4 py-3 text-sm font-medium text-emerald-800 shadow-lg">
          {toast}
        </div>
      )}

      {/* Header con botón destacado + Registrar gasto */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-slate-900 font-headline">
            Gestión y Control de Gastos
          </h1>
          <p className="text-sm text-slate-500 mt-0.5">
            Control de flujo de salida, costos fijos y variables para {currentBranch?.name || "la sucursal"}.
          </p>
        </div>
        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={reloadAll}
            className="inline-flex items-center gap-2 rounded-xl border border-slate-200 bg-white px-3.5 py-2.5 text-sm font-medium text-slate-700 shadow-sm transition hover:bg-slate-50"
            title="Recargar datos"
          >
            <RefreshCw className="h-4 w-4" />
            <span className="hidden sm:inline">Actualizar</span>
          </button>
          {canCreate && (
            <button
              type="button"
              onClick={handleOpenCreateModal}
              className="inline-flex items-center gap-2 rounded-xl bg-teal-700 hover:bg-teal-800 px-5 py-2.5 text-sm font-bold text-white shadow-sm transition-all active:scale-95"
            >
              <Plus className="h-4 w-4" />
              <span>+ Registrar gasto</span>
            </button>
          )}
        </div>
      </div>

      {/* 1. Fila de 2 tarjetas resumen */}
      <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
        {/* Card 1: Total Gastos del Mes */}
        <div className="bg-white rounded-2xl border border-slate-200 p-6 shadow-sm flex flex-col justify-between">
          <div>
            <div className="flex items-center justify-between mb-4">
              <span className="text-xs font-bold text-slate-500 uppercase tracking-wider">
                Total Gastos del Mes
              </span>
              <div className="w-10 h-10 rounded-xl bg-teal-50 flex items-center justify-center text-teal-700">
                <DollarSign className="h-5 w-5" />
              </div>
            </div>
            <div className="flex items-baseline gap-3">
              <span className="text-3xl font-extrabold text-slate-900 tracking-tight">
                {formatCOP(totalActiveExpenses)}
              </span>
            </div>
            <div className="mt-2.5 inline-flex items-center gap-1.5 px-3 py-1 rounded-full bg-slate-100 text-slate-600 text-xs font-medium">
              <Wallet className="h-3.5 w-3.5 text-teal-700" />
              <span>
                Presupuesto mensual: {formatCOP(monthlyBudget)} · {budgetExecutionPct.toFixed(1)}% ejecutado
              </span>
            </div>
          </div>
          {/* Barra de progreso de ejecución */}
          <div className="mt-6 pt-4 border-t border-slate-100">
            <div className="flex justify-between text-xs font-semibold text-slate-600 mb-2">
              <span>Ejecución del Límite de Gasto</span>
              <span className="text-teal-700 font-bold">{budgetExecutionPct.toFixed(1)}%</span>
            </div>
            <div className="w-full h-3 bg-slate-100 rounded-full overflow-hidden">
              <div
                className={cn(
                  "h-full rounded-full transition-all duration-500",
                  budgetExecutionPct > 90 ? "bg-rose-500" : "bg-teal-700",
                )}
                style={{ width: `${Math.min(100, budgetExecutionPct)}%` }}
              />
            </div>
            <p className="text-[11px] text-slate-400 mt-2">
              Saldo disponible presupuestado: {formatCOP(Math.max(0, monthlyBudget - totalActiveExpenses))}
            </p>
          </div>
        </div>

        {/* Card 2: Gasto más alto por categoría */}
        <div className="bg-white rounded-2xl border border-slate-200 p-6 shadow-sm flex flex-col justify-between">
          <div>
            <div className="flex items-center justify-between mb-4">
              <span className="text-xs font-bold text-slate-500 uppercase tracking-wider">
                Gasto más alto por categoría
              </span>
              <div className="w-10 h-10 rounded-xl bg-blue-50 flex items-center justify-center text-blue-700">
                <TrendingUp className="h-5 w-5" />
              </div>
            </div>
            <div className="flex items-baseline gap-3">
              <span className="text-3xl font-extrabold text-slate-900 tracking-tight">
                {topCategory ? formatCOP(topCategory.total) : "$ 0"}
              </span>
            </div>
            <p className="text-base font-semibold text-slate-800 mt-1">
              {topCategory ? topCategory.name : "Sin registros"}
            </p>
            <div className="mt-2.5 inline-flex items-center gap-1.5 px-3 py-1 rounded-full bg-amber-50 text-amber-700 border border-amber-200/50 text-xs font-medium">
              <TrendingUp className="h-3.5 w-3.5" />
              <span>
                Representa el {topCategoryPct.toFixed(1)}% del egreso total consolidado
              </span>
            </div>
          </div>
          <div className="mt-6 pt-4 border-t border-slate-100 flex items-center justify-between text-xs text-slate-500">
            <span>Categorías activas con movimientos:</span>
            <span className="font-bold text-slate-800">{categoryStats.length} categorías</span>
          </div>
        </div>
      </div>

      {/* 2. Gráfico de barras horizontal por categoría (ordenado de mayor a menor) */}
      {categoryStats.length > 0 && (
        <div className="bg-white rounded-2xl border border-slate-200 p-6 shadow-sm">
          <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-2 mb-6">
            <div>
              <h2 className="text-lg font-bold text-slate-900 font-headline">
                Distribución de Egresos por Categoría
              </h2>
              <p className="text-xs text-slate-500 mt-0.5">
                Gastos agrupados por categoría, ordenados de mayor a menor según el monto total ejecutado.
              </p>
            </div>
            <span className="text-xs font-semibold text-slate-600 bg-slate-100 px-3 py-1.5 rounded-lg self-start sm:self-auto">
              Total: {categoryStats.length} categorías
            </span>
          </div>

          <div className="space-y-4">
            {categoryStats.map((cat) => {
              const pct = totalActiveExpenses > 0 ? (cat.total / totalActiveExpenses) * 100 : 0;
              return (
                <div key={cat.name} className="space-y-1.5">
                  <div className="flex justify-between items-center text-xs font-semibold">
                    <span className="text-slate-800 flex items-center gap-2">
                      <span
                        className="w-2.5 h-2.5 rounded-full"
                        style={{ backgroundColor: cat.color }}
                      />
                      {cat.name}
                    </span>
                    <span className="text-slate-700 font-bold">
                      {formatCOP(cat.total)}{" "}
                      <span className="text-slate-400 font-normal">({pct.toFixed(1)}%)</span>
                    </span>
                  </div>
                  <div className="w-full h-3 bg-slate-100 rounded-full overflow-hidden">
                    <div
                      className="h-full rounded-full transition-all duration-500"
                      style={{
                        width: `${pct}%`,
                        backgroundColor: cat.color,
                      }}
                    />
                  </div>
                </div>
              );
            })}
          </div>
        </div>
      )}

      {/* 3. Filtros */}
      <div className="rounded-2xl border border-slate-200 bg-white p-4 shadow-sm">
        <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
          <div className="relative flex-1">
            <Search className="absolute left-3 top-2.5 h-4 w-4 text-slate-400" />
            <input
              type="text"
              placeholder="Buscar por concepto, N° de gasto, proveedor, método..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="w-full rounded-xl border border-slate-200 pl-9 pr-3 py-2 text-sm text-slate-900 placeholder-slate-400 focus:border-teal-700 focus:outline-none focus:ring-1 focus:ring-teal-700"
            />
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <select
              value={selectedCategoryId}
              onChange={(e) => setSelectedCategoryId(e.target.value)}
              className="rounded-xl border border-slate-200 bg-white px-3 py-2 text-xs font-medium text-slate-700 focus:border-teal-700 focus:outline-none"
            >
              <option value="ALL">Todas las categorías</option>
              {categories.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </select>

            {!USE_MOCKS && (
              <select
                value={selectedSupplierId}
                onChange={(e) => setSelectedSupplierId(e.target.value)}
                className="rounded-xl border border-slate-200 bg-white px-3 py-2 text-xs font-medium text-slate-700 focus:border-teal-700 focus:outline-none"
              >
                <option value="ALL">Todos los proveedores</option>
                {suppliers.map((s) => (
                  <option key={s.id} value={s.id}>
                    {s.name}
                  </option>
                ))}
              </select>
            )}

            {USE_MOCKS && (
              <select
                value={selectedPaymentMethod}
                onChange={(e) => setSelectedPaymentMethod(e.target.value)}
                className="rounded-xl border border-slate-200 bg-white px-3 py-2 text-xs font-medium text-slate-700 focus:border-teal-700 focus:outline-none"
              >
                <option value="ALL">Todos los métodos de pago</option>
                {PAYMENT_METHODS.map((m) => (
                  <option key={m} value={m}>
                    {m}
                  </option>
                ))}
              </select>
            )}

            <select
              value={selectedStatus}
              onChange={(e) => setSelectedStatus(e.target.value as ExpenseStatus | "ALL")}
              className="rounded-xl border border-slate-200 bg-white px-3 py-2 text-xs font-medium text-slate-700 focus:border-teal-700 focus:outline-none"
            >
              <option value="ALL">Todos los estados</option>
              <option value="PENDIENTE">Pendientes</option>
              <option value="APROBADO">Aprobados</option>
              <option value="PAGADO">Pagados</option>
              <option value="RECHAZADO">Rechazados</option>
              <option value="CANCELADO">Cancelados</option>
            </select>

            <div className="flex items-center gap-1">
              <input
                type="date"
                value={fromDate}
                onChange={(e) => setFromDate(e.target.value)}
                className="rounded-xl border border-slate-200 bg-white px-2 py-1.5 text-xs text-slate-700 focus:border-teal-700 focus:outline-none"
                title="Fecha desde"
              />
              <span className="text-xs text-slate-400">-</span>
              <input
                type="date"
                value={toDate}
                onChange={(e) => setToDate(e.target.value)}
                className="rounded-xl border border-slate-200 bg-white px-2 py-1.5 text-xs text-slate-700 focus:border-teal-700 focus:outline-none"
                title="Fecha hasta"
              />
            </div>
          </div>
        </div>
      </div>

      {/* 4. Tabla de historial de gastos */}
      <div className="overflow-hidden rounded-2xl border border-slate-200 bg-white shadow-sm">
        {loadingExpenses ? (
          <div className="flex h-64 items-center justify-center">
            <Loader2 className="h-8 w-8 animate-spin text-teal-700" />
          </div>
        ) : filteredExpenses.length === 0 ? (
          <div className="flex h-64 flex-col items-center justify-center p-6 text-center">
            <FileText className="h-10 w-10 text-slate-300" />
            <h3 className="mt-2 text-base font-semibold text-slate-900">
              No se encontraron gastos
            </h3>
            <p className="mt-1 text-sm text-slate-500">
              {search || selectedStatus !== "ALL" || selectedCategoryId !== "ALL"
                ? "Probá ajustando los filtros de búsqueda."
                : "Aún no hay gastos registrados para esta sucursal."}
            </p>
            {canCreate && (
              <button
                type="button"
                onClick={handleOpenCreateModal}
                className="mt-4 inline-flex items-center gap-2 rounded-xl bg-teal-700 hover:bg-teal-800 px-4 py-2 text-sm font-semibold text-white shadow-sm transition-all"
              >
                <Plus className="h-4 w-4" />
                <span>+ Registrar gasto</span>
              </button>
            )}
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left border-collapse text-sm text-slate-600">
              <thead>
                <tr className="bg-slate-50/75 border-b border-slate-200 text-[11px] font-semibold uppercase tracking-wider text-slate-500">
                  <th className="py-3.5 px-6">Fecha</th>
                  <th className="py-3.5 px-6">Concepto</th>
                  <th className="py-3.5 px-6">Categoría</th>
                  {!USE_MOCKS && <th className="py-3.5 px-6">Proveedor</th>}
                  <th className="py-3.5 px-6 text-right">Monto</th>
                  {USE_MOCKS && <th className="py-3.5 px-6">Método de pago</th>}
                  {USE_MOCKS && <th className="py-3.5 px-6 text-center">Comprobante</th>}
                  <th className="py-3.5 px-6 text-center">Estado</th>
                  <th className="py-3.5 px-6 text-right">Acciones</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100 text-sm">
                {filteredExpenses.map((exp) => {
                  const cfg = STATUS_CONFIG[exp.status];
                  const canCancelThis = canApprove || canCreate;
                  return (
                    <tr
                      key={exp.id}
                      className="transition-colors hover:bg-slate-50/80"
                    >
                      {/* Fecha */}
                      <td className="py-4 px-6 font-medium text-slate-900 whitespace-nowrap text-xs">
                        {formatExpenseDate(exp.expense_date)}
                      </td>

                      {/* Concepto */}
                      <td className="py-4 px-6">
                        <div className="font-semibold text-slate-900 line-clamp-1">
                          {exp.description}
                        </div>
                        <div className="text-[11px] text-slate-400">
                          {exp.document_number} {exp.supplier_name ? `· ${exp.supplier_name}` : ""}
                        </div>
                        {exp.reject_reason && (
                          <div className="text-xs text-rose-600 line-clamp-1 mt-0.5">
                            Motivo: {exp.reject_reason}
                          </div>
                        )}
                      </td>

                      {/* Categoría (Badge de color por tipo) */}
                      <td className="py-4 px-6 whitespace-nowrap">
                        <span
                          className="inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium border"
                          style={{
                            backgroundColor: `${exp.category_color}18`,
                            color: exp.category_color,
                            borderColor: `${exp.category_color}35`,
                          }}
                        >
                          <span
                            className="h-1.5 w-1.5 rounded-full"
                            style={{ backgroundColor: exp.category_color }}
                          />
                          {exp.category_name}
                        </span>
                      </td>

                      {/* Proveedor en modo real */}
                      {!USE_MOCKS && (
                        <td className="py-4 px-6 text-xs text-slate-600 whitespace-nowrap">
                          {exp.supplier_name || "-"}
                        </td>
                      )}

                      {/* Monto (COP) */}
                      <td className="py-4 px-6 text-right whitespace-nowrap font-bold text-slate-900">
                        <div>{formatCOP(Number(exp.total))}</div>
                        {Number(exp.tax_amount) > 0 && (
                          <div className="text-[11px] font-normal text-slate-400">
                            Imp: {formatCOP(Number(exp.tax_amount))}
                          </div>
                        )}
                      </td>

                      {/* Método de pago en modo demo */}
                      {USE_MOCKS && (
                        <td className="py-4 px-6 text-xs text-slate-600 whitespace-nowrap">
                          {exp.payment_method || "Efectivo / Caja Menor"}
                        </td>
                      )}

                      {/* Comprobante en modo demo */}
                      {USE_MOCKS && (
                        <td className="py-4 px-6 text-center whitespace-nowrap">
                          {exp.attachment_name ? (
                            <button
                              type="button"
                              onClick={() => showToast(`Visualizando soporte: ${exp.attachment_name}`)}
                              className="inline-flex items-center gap-1.5 px-2.5 py-1 rounded-lg text-xs font-medium text-teal-700 bg-teal-50 hover:bg-teal-100 transition-colors border border-teal-200/50"
                              title={exp.attachment_name}
                            >
                              <Paperclip className="h-3.5 w-3.5" />
                              <span className="max-w-[130px] truncate">{exp.attachment_name}</span>
                            </button>
                          ) : (
                            <span className="text-xs text-slate-300">-</span>
                          )}
                        </td>
                      )}

                      {/* Estado */}
                      <td className="py-4 px-6 text-center whitespace-nowrap">
                        <span
                          className={cn(
                            "inline-flex items-center gap-1.5 rounded-full border px-2.5 py-0.5 text-xs font-medium",
                            cfg.className,
                          )}
                        >
                          <span className={cn("h-1.5 w-1.5 rounded-full", cfg.dot)} />
                          {cfg.label}
                        </span>
                      </td>

                      {/* Acciones */}
                      <td className="py-4 px-6 text-right whitespace-nowrap">
                        <div className="flex items-center justify-end gap-1.5">
                          {exp.status === "PENDIENTE" && (
                            <>
                              {canApprove && (
                                <button
                                  type="button"
                                  onClick={() => handleApprove(exp)}
                                  className="rounded-lg px-2.5 py-1 text-xs font-medium text-emerald-700 hover:bg-emerald-50 transition-colors"
                                >
                                  Aprobar
                                </button>
                              )}
                              {canApprove && (
                                <button
                                  type="button"
                                  onClick={() => handleOpenReject(exp)}
                                  className="rounded-lg px-2.5 py-1 text-xs font-medium text-rose-700 hover:bg-rose-50 transition-colors"
                                >
                                  Rechazar
                                </button>
                              )}
                              {canCancelThis && (
                                <button
                                  type="button"
                                  onClick={() => handleCancel(exp)}
                                  className="rounded-lg px-2 py-1 text-xs font-medium text-slate-500 hover:bg-slate-100 transition-colors"
                                >
                                  Cancelar
                                </button>
                              )}
                            </>
                          )}

                          {exp.status === "APROBADO" && canPay && (
                            <button
                              type="button"
                              onClick={() => handleOpenPayment(exp)}
                              className="inline-flex items-center gap-1 rounded-lg bg-teal-50 px-2.5 py-1 text-xs font-semibold text-teal-700 hover:bg-teal-100 transition-colors"
                            >
                              <Banknote className="h-3.5 w-3.5" />
                              <span>Pagar</span>
                            </button>
                          )}

                          {exp.status === "PAGADO" && (
                            <span className="inline-flex items-center gap-1 text-xs text-emerald-600 font-semibold">
                              <CheckCircle2 className="h-3.5 w-3.5" />
                              <span>Pagado</span>
                            </span>
                          )}
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </div>

      {/* Modal: Registrar nuevo gasto */}
      <Dialog open={isCreateModalOpen} onOpenChange={setIsCreateModalOpen}>
        <DialogContent className="max-w-lg">
          <DialogHeader>
            <DialogTitle className="font-headline text-lg font-bold text-slate-900">
              Registrar nuevo gasto operativo
            </DialogTitle>
            <DialogDescription className="text-xs text-slate-500">
              Registra un egreso de caja o cuenta bancaria con su comprobante de respaldo.
            </DialogDescription>
          </DialogHeader>

          <form onSubmit={handleCreateSubmit} className="space-y-4 pt-1">
            {createError && (
              <div className="rounded-xl border border-rose-200 bg-rose-50 p-3 text-xs text-rose-700">
                {createError}
              </div>
            )}

            {/* Concepto del gasto */}
            <div>
              <label className="block text-xs font-bold text-slate-700 mb-1">
                Concepto del gasto *
              </label>
              <input
                type="text"
                value={formDescription}
                onChange={(e) => setFormDescription(e.target.value)}
                required
                placeholder="ej. Pago servicio internet fibra óptica"
                className="w-full rounded-xl border border-slate-200 px-3.5 py-2.5 text-sm text-slate-900 placeholder-slate-400 focus:border-teal-700 focus:outline-none focus:ring-1 focus:ring-teal-700"
              />
            </div>

            {/* Grid 2 Cols: Categoría & Monto */}
            <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
              <div>
                <label className="block text-xs font-bold text-slate-700 mb-1">
                  Categoría *
                </label>
                <select
                  value={formCategoryId}
                  onChange={(e) => setFormCategoryId(e.target.value)}
                  required
                  className="w-full rounded-xl border border-slate-200 bg-white px-3 py-2.5 text-sm text-slate-800 focus:border-teal-700 focus:outline-none"
                >
                  {categories.map((c) => (
                    <option key={c.id} value={c.id}>
                      {c.name}
                    </option>
                  ))}
                </select>
              </div>

              <div>
                <label className="block text-xs font-bold text-slate-700 mb-1">
                  Monto en COP *
                </label>
                <MoneyInput
                  value={formAmount}
                  onChange={setFormAmount}
                  placeholder="0 COP"
                />
              </div>
            </div>

            {/* Fecha de pago & Método de pago */}
            <div className={USE_MOCKS ? "grid grid-cols-1 sm:grid-cols-2 gap-4" : ""}>
              <div>
                <label className="block text-xs font-bold text-slate-700 mb-1">
                  Fecha de pago *
                </label>
                <input
                  type="date"
                  value={formDate}
                  onChange={(e) => setFormDate(e.target.value)}
                  required
                  className="w-full rounded-xl border border-slate-200 px-3 py-2 text-sm text-slate-800 focus:border-teal-700 focus:outline-none"
                />
              </div>

              {USE_MOCKS && (
                <div>
                  <label className="block text-xs font-bold text-slate-700 mb-1">
                    Método de pago *
                  </label>
                  <select
                    value={formPaymentMethod}
                    onChange={(e) => setFormPaymentMethod(e.target.value)}
                    className="w-full rounded-xl border border-slate-200 bg-white px-3 py-2.5 text-sm text-slate-800 focus:border-teal-700 focus:outline-none"
                  >
                    {PAYMENT_METHODS.map((m) => (
                      <option key={m} value={m}>
                        {m}
                      </option>
                    ))}
                  </select>
                </div>
              )}
            </div>

            {/* Proveedor opcional */}
            <div>
              <label className="block text-xs font-medium text-slate-700 mb-1">
                Proveedor (opcional)
              </label>
              <select
                value={formSupplierId}
                onChange={(e) => setFormSupplierId(e.target.value)}
                className="w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-800 focus:border-teal-700 focus:outline-none"
              >
                <option value="">Ninguno / Pago directo</option>
                {suppliers.map((s) => (
                  <option key={s.id} value={s.id}>
                    {s.name}
                  </option>
                ))}
              </select>
            </div>

            {/* Adjuntar comprobante (dropzone simulado solo en modo demo) */}
            {USE_MOCKS && (
              <div>
                <label className="block text-xs font-bold text-slate-700 mb-1">
                  Adjuntar comprobante o soporte (opcional)
                </label>
                <input
                  type="file"
                  ref={fileInputRef}
                  onChange={handleFileSelect}
                  className="hidden"
                  accept=".pdf,.png,.jpg,.jpeg"
                />

                {formAttachmentName ? (
                  <div className="flex items-center justify-between rounded-xl border border-teal-200 bg-teal-50/60 p-3 text-xs">
                    <div className="flex items-center gap-2">
                      <Paperclip className="h-4 w-4 text-teal-700" />
                      <span className="font-semibold text-teal-900">{formAttachmentName}</span>
                      <span className="text-[11px] text-teal-600">(1.2 MB)</span>
                    </div>
                    <button
                      type="button"
                      onClick={() => setFormAttachmentName("")}
                      className="p-1 text-slate-400 hover:text-slate-600 rounded-lg hover:bg-teal-100/50 transition-colors"
                    >
                      <X className="h-4 w-4" />
                    </button>
                  </div>
                ) : (
                  <div
                    onClick={() => fileInputRef.current?.click()}
                    className="border-2 border-dashed border-slate-200 hover:border-teal-700 rounded-xl p-4 text-center bg-slate-50/50 hover:bg-teal-50/20 transition-all cursor-pointer"
                  >
                    <div className="flex flex-col items-center gap-1.5">
                      <div className="w-9 h-9 rounded-full bg-slate-100 flex items-center justify-center text-slate-500">
                        <Upload className="h-4 w-4" />
                      </div>
                      <p className="text-xs font-semibold text-slate-700">
                        Haz clic para subir o arrastra un archivo
                      </p>
                      <p className="text-[11px] text-slate-400">
                        PDF, JPG o PNG hasta 10MB
                      </p>
                      <button
                        type="button"
                        onClick={(e) => {
                          e.stopPropagation();
                          handleSimulateAttachment();
                        }}
                        className="mt-1 text-[11px] text-teal-700 underline font-medium hover:text-teal-800"
                      >
                        (O simular adjunto rápido)
                      </button>
                    </div>
                  </div>
                )}
              </div>
            )}

            <div className="rounded-xl bg-slate-50 p-3 text-xs text-slate-600 flex justify-between font-semibold">
              <span>Total a registrar:</span>
              <span className="text-slate-900 font-bold text-sm">
                {formatCOP(formAmount + formTaxAmount)}
              </span>
            </div>

            <div className="flex justify-end gap-2 pt-2 border-t border-slate-100">
              <button
                type="button"
                onClick={() => setIsCreateModalOpen(false)}
                className="rounded-xl border border-slate-200 px-4 py-2 text-sm font-semibold text-slate-600 hover:bg-slate-50 transition-colors"
              >
                Cancelar
              </button>
              <button
                type="submit"
                disabled={createBusy}
                className="inline-flex items-center gap-2 rounded-xl bg-teal-700 hover:bg-teal-800 px-5 py-2 text-sm font-bold text-white shadow-sm transition-all disabled:opacity-50"
              >
                {createBusy && <Loader2 className="h-4 w-4 animate-spin" />}
                Guardar gasto
              </button>
            </div>
          </form>
        </DialogContent>
      </Dialog>

      {/* Modal: Rechazar gasto */}
      <Dialog open={!!rejectModalExpense} onOpenChange={(o) => !o && setRejectModalExpense(null)}>
        <DialogContent className="max-w-sm">
          <DialogHeader>
            <DialogTitle>Rechazar gasto</DialogTitle>
            <DialogDescription>
              {rejectModalExpense?.document_number} - {rejectModalExpense?.description}
            </DialogDescription>
          </DialogHeader>

          <form onSubmit={handleRejectSubmit} className="space-y-4 pt-2">
            {rejectError && (
              <div className="rounded-xl border border-rose-200 bg-rose-50 p-3 text-xs text-rose-700">
                {rejectError}
              </div>
            )}

            <div>
              <label className="block text-xs font-medium text-slate-700">
                Motivo del rechazo *
              </label>
              <textarea
                value={rejectReason}
                onChange={(e) => setRejectReason(e.target.value)}
                required
                rows={3}
                placeholder="Escribí por qué se rechaza este gasto..."
                className="mt-1 w-full rounded-xl border border-slate-200 px-3 py-2 text-sm text-slate-900 placeholder-slate-400 focus:border-rose-500 focus:outline-none"
              />
            </div>

            <div className="flex justify-end gap-2 pt-2">
              <button
                type="button"
                onClick={() => setRejectModalExpense(null)}
                className="rounded-xl border border-slate-200 px-4 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
              >
                Cerrar
              </button>
              <button
                type="submit"
                disabled={rejectBusy}
                className="inline-flex items-center gap-2 rounded-xl bg-rose-600 hover:bg-rose-700 px-4 py-2 text-sm font-medium text-white shadow-sm disabled:opacity-50"
              >
                {rejectBusy && <Loader2 className="h-4 w-4 animate-spin" />}
                Confirmar rechazo
              </button>
            </div>
          </form>
        </DialogContent>
      </Dialog>

      {/* Modal: Pagar cuenta por pagar del gasto */}
      <Dialog
        open={!!paymentModalExpense}
        onOpenChange={(o) => !o && setPaymentModalExpense(null)}
      >
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>Pagar gasto</DialogTitle>
            <DialogDescription>
              Desembolso para {paymentModalExpense?.document_number} (
              {paymentModalExpense?.description})
            </DialogDescription>
          </DialogHeader>

          <form onSubmit={handlePaymentSubmit} className="space-y-4 pt-2">
            {paymentError && (
              <div className="rounded-xl border border-rose-200 bg-rose-50 p-3 text-xs text-rose-700">
                {paymentError}
              </div>
            )}

            <div>
              <label className="block text-xs font-medium text-slate-700">
                Monto a desembolsar *
              </label>
              <MoneyInput
                value={paymentAmount}
                onChange={setPaymentAmount}
                className="mt-1"
              />
            </div>

            <div>
              <label className="block text-xs font-medium text-slate-700">
                Método de pago *
              </label>
              <select
                value={payableMethod}
                onChange={(e) => setPayableMethod(e.target.value as PayablePaymentMethod)}
                className="mt-1 w-full rounded-xl border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 focus:border-teal-700 focus:outline-none"
              >
                <option value="EFECTIVO">Efectivo (baja caja del turno actual)</option>
                <option value="TRANSFERENCIA">Transferencia bancaria</option>
                <option value="CHEQUE">Cheque</option>
              </select>
            </div>

            {payableMethod !== "EFECTIVO" && (
              <div>
                <label className="block text-xs font-medium text-slate-700">
                  Referencia / N° Comprobante *
                </label>
                <input
                  type="text"
                  value={paymentReference}
                  onChange={(e) => setPaymentReference(e.target.value)}
                  placeholder="Ej. TRANSF-982348..."
                  required
                  className="mt-1 w-full rounded-xl border border-slate-200 px-3 py-2 text-sm text-slate-900 placeholder-slate-400 focus:border-teal-700 focus:outline-none"
                />
              </div>
            )}

            <div className="flex justify-end gap-2 pt-2 border-t border-slate-100">
              <button
                type="button"
                onClick={() => setPaymentModalExpense(null)}
                className="rounded-xl border border-slate-200 px-4 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
              >
                Cancelar
              </button>
              <button
                type="submit"
                disabled={paymentBusy}
                className="inline-flex items-center gap-2 rounded-xl bg-teal-700 hover:bg-teal-800 px-4 py-2 text-sm font-medium text-white shadow-sm disabled:opacity-50"
              >
                {paymentBusy && <Loader2 className="h-4 w-4 animate-spin" />}
                Registrar desembolso
              </button>
            </div>
          </form>
        </DialogContent>
      </Dialog>
    </div>
  );
}
