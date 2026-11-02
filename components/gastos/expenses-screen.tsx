"use client";

import { useMemo, useState } from "react";
import {
  Banknote,
  CheckCircle2,
  Clock,
  DollarSign,
  FileText,
  Loader2,
  Plus,
  RefreshCw,
  Search,
} from "lucide-react";

import { StatCard } from "@/components/nexa/stat-card";
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
  const [createBusy, setCreateBusy] = useState(false);
  const [createError, setCreateError] = useState<string | null>(null);

  // Formulario rechazo
  const [rejectReason, setRejectReason] = useState("");
  const [rejectBusy, setRejectBusy] = useState(false);
  const [rejectError, setRejectError] = useState<string | null>(null);

  // Formulario pago de cuenta por pagar vinculada
  const [paymentAmount, setPaymentAmount] = useState<number>(0);
  const [paymentMethod, setPaymentMethod] = useState<PayablePaymentMethod>("EFECTIVO");
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
    data: summary,
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
      if (fromDate && e.expense_date < fromDate) return false;
      if (toDate && e.expense_date > toDate) return false;
      if (search.trim()) {
        const q = search.toLowerCase();
        const matchesDoc = e.document_number.toLowerCase().includes(q);
        const matchesDesc = e.description.toLowerCase().includes(q);
        const matchesCat = e.category_name.toLowerCase().includes(q);
        const matchesSupp = e.supplier_name?.toLowerCase().includes(q) ?? false;
        if (!matchesDoc && !matchesDesc && !matchesCat && !matchesSupp) return false;
      }
      return true;
    });
  }, [expenses, selectedStatus, selectedCategoryId, selectedSupplierId, fromDate, toDate, search]);

  // Agrupación por categoría para gráfico de barras horizontal
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

  const maxCategoryTotal = useMemo(() => {
    return categoryStats.length > 0 ? categoryStats[0].total : 0;
  }, [categoryStats]);

  // Manejar creación
  const handleOpenCreateModal = () => {
    setFormCategoryId(categories[0]?.id || "");
    setFormSupplierId("");
    setFormDescription("");
    setFormAmount(0);
    setFormTaxAmount(0);
    setFormDate(new Date().toISOString().slice(0, 10));
    setCreateError(null);
    setIsCreateModalOpen(true);
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
    setPaymentMethod("EFECTIVO");
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
    if ((paymentMethod === "TRANSFERENCIA" || paymentMethod === "CHEQUE") && !paymentReference.trim()) {
      setPaymentError("La referencia es obligatoria para transferencias o cheques.");
      return;
    }

    try {
      setPaymentBusy(true);
      setPaymentError(null);
      const input: CreatePayablePaymentInput = {
        id: crypto.randomUUID(),
        amount: paymentAmount.toFixed(2),
        method: paymentMethod,
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

      {/* Header */}
      <div className="flex flex-col gap-4 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-2xl font-bold tracking-tight text-slate-900">
            Gastos Operativos
          </h1>
          <p className="text-sm text-slate-500">
            Registro, aprobación y control de gastos para {currentBranch?.name || "la sucursal"}.
          </p>
        </div>
        <div className="flex items-center gap-3">
          <button
            type="button"
            onClick={reloadAll}
            className="inline-flex items-center gap-2 rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm font-medium text-slate-700 shadow-sm transition hover:bg-slate-50"
            title="Recargar datos"
          >
            <RefreshCw className="h-4 w-4" />
            <span className="hidden sm:inline">Actualizar</span>
          </button>
          {canCreate && (
            <button
              type="button"
              onClick={handleOpenCreateModal}
              className="inline-flex items-center gap-2 rounded-lg bg-emerald-600 px-4 py-2 text-sm font-medium text-white shadow-sm transition hover:bg-emerald-700"
            >
              <Plus className="h-4 w-4" />
              <span>Registrar gasto</span>
            </button>
          )}
        </div>
      </div>

      {/* StatCards del mes */}
      <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-4">
        <StatCard
          title="Total mes (Aprobados/Pagados)"
          value={formatCOP(Number(summary?.total_approved_month ?? 0))}
          caption={`${summary?.count_approved ?? 0} gastos contabilizados`}
          icon={DollarSign}
          tone="success"
        />
        <StatCard
          title="Pendientes por aprobar"
          value={formatCOP(Number(summary?.total_pending_month ?? 0))}
          caption={`${summary?.count_pending ?? 0} gastos pendientes`}
          icon={Clock}
          tone="warning"
        />
        <StatCard
          title="Pagados en el mes"
          value={formatCOP(Number(summary?.total_paid_month ?? 0))}
          caption="Desembolsos completados"
          icon={CheckCircle2}
          tone="default"
        />
        <StatCard
          title="Gasto más alto"
          value={categoryStats.length > 0 ? formatCOP(categoryStats[0].total) : "$ 0"}
          caption={categoryStats.length > 0 ? categoryStats[0].name : "Sin registros"}
          icon={Banknote}
          tone="default"
        />
      </div>

      {/* Gráfico de barras horizontal: gastos por categoría */}
      {categoryStats.length > 0 && (
        <div className="rounded-xl border border-slate-200 bg-white p-5 shadow-sm">
          <h2 className="text-base font-semibold text-slate-900">
            Distribución por categoría (Gastos activos)
          </h2>
          <p className="mb-4 text-xs text-slate-500">
            Comparativa de desembolsos aprobados y pagados por categoría.
          </p>
          <div className="space-y-3">
            {categoryStats.map((cat) => {
              const pct = maxCategoryTotal > 0 ? (cat.total / maxCategoryTotal) * 100 : 0;
              return (
                <div key={cat.name} className="space-y-1">
                  <div className="flex items-center justify-between text-xs font-medium">
                    <span className="flex items-center gap-2 text-slate-700">
                      <span
                        className="inline-block h-3 w-3 rounded-full"
                        style={{ backgroundColor: cat.color }}
                      />
                      {cat.name}
                    </span>
                    <span className="text-slate-900">{formatCOP(cat.total)}</span>
                  </div>
                  <div className="h-2 w-full overflow-hidden rounded-full bg-slate-100">
                    <div
                      className="h-full rounded-full transition-all duration-300"
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

      {/* Filtros */}
      <div className="rounded-xl border border-slate-200 bg-white p-4 shadow-sm">
        <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
          <div className="relative flex-1">
            <Search className="absolute left-3 top-2.5 h-4 w-4 text-slate-400" />
            <input
              type="text"
              placeholder="Buscar por concepto, N° de gasto, proveedor..."
              value={search}
              onChange={(e) => setSearch(e.target.value)}
              className="w-full rounded-lg border border-slate-200 pl-9 pr-3 py-2 text-sm text-slate-900 placeholder-slate-400 focus:border-emerald-500 focus:outline-none focus:ring-1 focus:ring-emerald-500"
            />
          </div>

          <div className="flex flex-wrap items-center gap-2">
            <select
              value={selectedStatus}
              onChange={(e) => setSelectedStatus(e.target.value as ExpenseStatus | "ALL")}
              className="rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm text-slate-700 focus:border-emerald-500 focus:outline-none"
            >
              <option value="ALL">Todos los estados</option>
              <option value="PENDIENTE">Pendientes</option>
              <option value="APROBADO">Aprobados</option>
              <option value="PAGADO">Pagados</option>
              <option value="RECHAZADO">Rechazados</option>
              <option value="CANCELADO">Cancelados</option>
            </select>

            <select
              value={selectedCategoryId}
              onChange={(e) => setSelectedCategoryId(e.target.value)}
              className="rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm text-slate-700 focus:border-emerald-500 focus:outline-none"
            >
              <option value="ALL">Todas las categorías</option>
              {categories.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.name}
                </option>
              ))}
            </select>

            <select
              value={selectedSupplierId}
              onChange={(e) => setSelectedSupplierId(e.target.value)}
              className="rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm text-slate-700 focus:border-emerald-500 focus:outline-none"
            >
              <option value="ALL">Todos los proveedores</option>
              {suppliers.map((s) => (
                <option key={s.id} value={s.id}>
                  {s.name}
                </option>
              ))}
            </select>

            <div className="flex items-center gap-1">
              <input
                type="date"
                value={fromDate}
                onChange={(e) => setFromDate(e.target.value)}
                className="rounded-lg border border-slate-200 bg-white px-2 py-1.5 text-xs text-slate-700 focus:border-emerald-500 focus:outline-none"
                title="Fecha desde"
              />
              <span className="text-xs text-slate-400">-</span>
              <input
                type="date"
                value={toDate}
                onChange={(e) => setToDate(e.target.value)}
                className="rounded-lg border border-slate-200 bg-white px-2 py-1.5 text-xs text-slate-700 focus:border-emerald-500 focus:outline-none"
                title="Fecha hasta"
              />
            </div>
          </div>
        </div>
      </div>

      {/* Tabla de gastos */}
      <div className="overflow-hidden rounded-xl border border-slate-200 bg-white shadow-sm">
        {loadingExpenses ? (
          <div className="flex h-64 items-center justify-center">
            <Loader2 className="h-8 w-8 animate-spin text-emerald-600" />
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
                className="mt-4 inline-flex items-center gap-2 rounded-lg bg-emerald-600 px-4 py-2 text-sm font-medium text-white shadow-sm transition hover:bg-emerald-700"
              >
                <Plus className="h-4 w-4" />
                <span>Registrar primer gasto</span>
              </button>
            )}
          </div>
        ) : (
          <div className="overflow-x-auto">
            <table className="w-full text-left text-sm text-slate-600">
              <thead className="border-b border-slate-200 bg-slate-50 text-xs font-semibold uppercase tracking-wider text-slate-600">
                <tr>
                  <th className="px-4 py-3">Gasto</th>
                  <th className="px-4 py-3">Fecha</th>
                  <th className="px-4 py-3">Concepto / Detalle</th>
                  <th className="px-4 py-3">Categoría</th>
                  <th className="px-4 py-3">Proveedor</th>
                  <th className="px-4 py-3 text-right">Monto</th>
                  <th className="px-4 py-3 text-center">Estado</th>
                  <th className="px-4 py-3 text-right">Acciones</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-slate-100">
                {filteredExpenses.map((exp) => {
                  const cfg = STATUS_CONFIG[exp.status];
                  const canCancelThis = canApprove || canCreate;
                  return (
                    <tr
                      key={exp.id}
                      className="transition-colors hover:bg-slate-50/80"
                    >
                      <td className="whitespace-nowrap px-4 py-3 font-medium text-slate-900">
                        {exp.document_number}
                      </td>
                      <td className="whitespace-nowrap px-4 py-3 text-xs text-slate-500">
                        {exp.expense_date}
                      </td>
                      <td className="px-4 py-3">
                        <div className="font-medium text-slate-900 line-clamp-1">
                          {exp.description}
                        </div>
                        {exp.reject_reason && (
                          <div className="text-xs text-rose-600 line-clamp-1">
                            Motivo: {exp.reject_reason}
                          </div>
                        )}
                      </td>
                      <td className="whitespace-nowrap px-4 py-3">
                        <span
                          className="inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium"
                          style={{
                            backgroundColor: `${exp.category_color}1A`,
                            color: exp.category_color,
                          }}
                        >
                          <span
                            className="h-1.5 w-1.5 rounded-full"
                            style={{ backgroundColor: exp.category_color }}
                          />
                          {exp.category_name}
                        </span>
                      </td>
                      <td className="whitespace-nowrap px-4 py-3 text-xs text-slate-600">
                        {exp.supplier_name || "-"}
                      </td>
                      <td className="whitespace-nowrap px-4 py-3 text-right">
                        <div className="font-semibold text-slate-900">
                          {formatCOP(Number(exp.total))}
                        </div>
                        {Number(exp.tax_amount) > 0 && (
                          <div className="text-xs text-slate-400">
                            Imp: {formatCOP(Number(exp.tax_amount))}
                          </div>
                        )}
                      </td>
                      <td className="whitespace-nowrap px-4 py-3 text-center">
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
                      <td className="whitespace-nowrap px-4 py-3 text-right">
                        <div className="flex items-center justify-end gap-1.5">
                          {exp.status === "PENDIENTE" && (
                            <>
                              {canApprove && (
                                <button
                                  type="button"
                                  onClick={() => handleApprove(exp)}
                                  className="rounded px-2 py-1 text-xs font-medium text-emerald-700 hover:bg-emerald-50"
                                >
                                  Aprobar
                                </button>
                              )}
                              {canApprove && (
                                <button
                                  type="button"
                                  onClick={() => handleOpenReject(exp)}
                                  className="rounded px-2 py-1 text-xs font-medium text-rose-700 hover:bg-rose-50"
                                >
                                  Rechazar
                                </button>
                              )}
                              {canCancelThis && (
                                <button
                                  type="button"
                                  onClick={() => handleCancel(exp)}
                                  className="rounded px-2 py-1 text-xs font-medium text-slate-600 hover:bg-slate-100"
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
                              className="inline-flex items-center gap-1 rounded bg-emerald-50 px-2 py-1 text-xs font-medium text-emerald-700 hover:bg-emerald-100"
                            >
                              <Banknote className="h-3 w-3" />
                              <span>Pagar</span>
                            </button>
                          )}

                          {exp.status === "PAGADO" && (
                            <span className="text-xs text-emerald-600 font-medium">
                              Desembolsado
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

      {/* Modal: Registrar gasto */}
      <Dialog open={isCreateModalOpen} onOpenChange={setIsCreateModalOpen}>
        <DialogContent className="max-w-md">
          <DialogHeader>
            <DialogTitle>Registrar nuevo gasto</DialogTitle>
            <DialogDescription>
              Ingresá el detalle del gasto operativo para su aprobación y pago.
            </DialogDescription>
          </DialogHeader>

          <form onSubmit={handleCreateSubmit} className="space-y-4 pt-2">
            {createError && (
              <div className="rounded-lg border border-rose-200 bg-rose-50 p-3 text-xs text-rose-700">
                {createError}
              </div>
            )}

            <div>
              <label className="block text-xs font-medium text-slate-700">
                Categoría *
              </label>
              <select
                value={formCategoryId}
                onChange={(e) => setFormCategoryId(e.target.value)}
                required
                className="mt-1 w-full rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 focus:border-emerald-500 focus:outline-none"
              >
                {categories.map((c) => (
                  <option key={c.id} value={c.id}>
                    {c.name}
                  </option>
                ))}
              </select>
            </div>

            <div>
              <label className="block text-xs font-medium text-slate-700">
                Proveedor (opcional)
              </label>
              <select
                value={formSupplierId}
                onChange={(e) => setFormSupplierId(e.target.value)}
                className="mt-1 w-full rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 focus:border-emerald-500 focus:outline-none"
              >
                <option value="">Ninguno / Pago directo</option>
                {suppliers.map((s) => (
                  <option key={s.id} value={s.id}>
                    {s.name}
                  </option>
                ))}
              </select>
            </div>

            <div>
              <label className="block text-xs font-medium text-slate-700">
                Concepto / Descripción *
              </label>
              <textarea
                value={formDescription}
                onChange={(e) => setFormDescription(e.target.value)}
                required
                rows={2}
                placeholder="Ej. Arriendo local noviembre..."
                className="mt-1 w-full rounded-lg border border-slate-200 px-3 py-2 text-sm text-slate-900 placeholder-slate-400 focus:border-emerald-500 focus:outline-none"
              />
            </div>

            <div className="grid grid-cols-2 gap-3">
              <div>
                <label className="block text-xs font-medium text-slate-700">
                  Monto neto *
                </label>
                <MoneyInput
                  value={formAmount}
                  onChange={setFormAmount}
                  className="mt-1"
                />
              </div>

              <div>
                <label className="block text-xs font-medium text-slate-700">
                  Impuesto (IVA/INC)
                </label>
                <MoneyInput
                  value={formTaxAmount}
                  onChange={setFormTaxAmount}
                  className="mt-1"
                />
              </div>
            </div>

            <div className="rounded-lg bg-slate-50 p-2.5 text-xs text-slate-600 flex justify-between font-semibold">
              <span>Total a pagar:</span>
              <span className="text-slate-900 font-bold">
                {formatCOP(formAmount + formTaxAmount)}
              </span>
            </div>

            <div>
              <label className="block text-xs font-medium text-slate-700">
                Fecha del gasto *
              </label>
              <input
                type="date"
                value={formDate}
                onChange={(e) => setFormDate(e.target.value)}
                required
                className="mt-1 w-full rounded-lg border border-slate-200 px-3 py-2 text-sm text-slate-900 focus:border-emerald-500 focus:outline-none"
              />
            </div>

            <div className="flex justify-end gap-2 pt-2">
              <button
                type="button"
                onClick={() => setIsCreateModalOpen(false)}
                className="rounded-lg border border-slate-200 px-4 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
              >
                Cancelar
              </button>
              <button
                type="submit"
                disabled={createBusy}
                className="inline-flex items-center gap-2 rounded-lg bg-emerald-600 px-4 py-2 text-sm font-medium text-white shadow-sm hover:bg-emerald-700 disabled:opacity-50"
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
              <div className="rounded-lg border border-rose-200 bg-rose-50 p-3 text-xs text-rose-700">
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
                className="mt-1 w-full rounded-lg border border-slate-200 px-3 py-2 text-sm text-slate-900 placeholder-slate-400 focus:border-rose-500 focus:outline-none"
              />
            </div>

            <div className="flex justify-end gap-2 pt-2">
              <button
                type="button"
                onClick={() => setRejectModalExpense(null)}
                className="rounded-lg border border-slate-200 px-4 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
              >
                Cerrar
              </button>
              <button
                type="submit"
                disabled={rejectBusy}
                className="inline-flex items-center gap-2 rounded-lg bg-rose-600 px-4 py-2 text-sm font-medium text-white shadow-sm hover:bg-rose-700 disabled:opacity-50"
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
              <div className="rounded-lg border border-rose-200 bg-rose-50 p-3 text-xs text-rose-700">
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
                value={paymentMethod}
                onChange={(e) => setPaymentMethod(e.target.value as PayablePaymentMethod)}
                className="mt-1 w-full rounded-lg border border-slate-200 bg-white px-3 py-2 text-sm text-slate-900 focus:border-emerald-500 focus:outline-none"
              >
                <option value="EFECTIVO">Efectivo (baja caja del turno actual)</option>
                <option value="TRANSFERENCIA">Transferencia bancaria</option>
                <option value="CHEQUE">Cheque</option>
              </select>
            </div>

            {paymentMethod !== "EFECTIVO" && (
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
                  className="mt-1 w-full rounded-lg border border-slate-200 px-3 py-2 text-sm text-slate-900 placeholder-slate-400 focus:border-emerald-500 focus:outline-none"
                />
              </div>
            )}

            <div className="flex justify-end gap-2 pt-2">
              <button
                type="button"
                onClick={() => setPaymentModalExpense(null)}
                className="rounded-lg border border-slate-200 px-4 py-2 text-sm font-medium text-slate-700 hover:bg-slate-50"
              >
                Cancelar
              </button>
              <button
                type="submit"
                disabled={paymentBusy}
                className="inline-flex items-center gap-2 rounded-lg bg-emerald-600 px-4 py-2 text-sm font-medium text-white shadow-sm hover:bg-emerald-700 disabled:opacity-50"
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
