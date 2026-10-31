import { Banknote, CreditCard, QrCode, type LucideIcon } from "lucide-react";

import type { PaymentMethod } from "@/lib/api";

export const PAYMENT_METHODS: { id: PaymentMethod; label: string; icon: LucideIcon; tone: string }[] = [
  { id: "EFECTIVO", label: "Efectivo", icon: Banknote, tone: "text-emerald-600" },
  { id: "DATAFONO", label: "Datáfono", icon: CreditCard, tone: "text-sky-600" },
  { id: "QR", label: "Nequi / QR", icon: QrCode, tone: "text-violet-600" },
];
