"use client";

import { useMemo, useState } from "react";
import {
  AlertCircle,
  CheckCircle2,
  Clock,
  MapPin,
  MessageCircle,
  Phone,
  Search,
  ShoppingBag,
  ShoppingCart,
  Store,
  Truck,
  UtensilsCrossed,
  X,
  XCircle,
} from "lucide-react";

import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import { formatCOP } from "@/lib/format";
import { cn } from "@/lib/utils";

export type OrderChannel = "MOSTRADOR" | "WHATSAPP" | "TIENDA_ONLINE";

export type OmnichannelOrderStatus =
  | "CREADA"
  | "EN_PREPARACION"
  | "LISTA"
  | "DESPACHADA"
  | "CANCELADA";

export interface MockOrderItem {
  qty: number;
  name: string;
  category: string;
  unitPrice: number;
}

export interface MockOrder {
  id: string;
  orderNumber: string;
  channel: OrderChannel;
  status: OmnichannelOrderStatus;
  createdAt: string; // e.g. "Hace 18 min"
  timeReceived: string; // e.g. "11:42 AM"
  customer: {
    name: string;
    phone: string;
    frequent?: boolean;
    address?: string;
    addressNotes?: string;
    deliveryType?: "DOMICILIO_PROPIO" | "EXPRESS" | "PARA_LLEVAR" | "MESA";
    tableOrTakeoutInfo?: string;
  };
  items: MockOrderItem[];
  deliveryFee: number;
  payment: {
    method: string;
    status: "PAGADO" | "POR_VERIFICAR" | "PENDIENTE";
    label: string;
  };
  cashierName?: string;
}

const INITIAL_MOCK_ORDERS: MockOrder[] = [
  {
    id: "ord-1",
    orderNumber: "#NX-1048",
    channel: "WHATSAPP",
    status: "EN_PREPARACION",
    createdAt: "Hace 18 min",
    timeReceived: "11:42 AM",
    customer: {
      name: "María Camila Restrepo",
      phone: "310 458 9201",
      frequent: true,
      address: "Cra 43A #12-45 Apto 302, El Poblado",
      addressNotes: "Tocar citófono al llegar",
      deliveryType: "DOMICILIO_PROPIO",
    },
    items: [
      { qty: 2, name: "Coca-Cola 400 ml", category: "Bebidas frías", unitPrice: 3500 },
      { qty: 1, name: "Café Sello Rojo 250 g", category: "Grano molido", unitPrice: 9500 },
      { qty: 1, name: "Six pack Águila", category: "Cerveza", unitPrice: 17500 },
      { qty: 2, name: "Empanada de Carne", category: "Frito típico", unitPrice: 4000 },
      { qty: 1, name: "Tostada Aguacate", category: "Especialidad", unitPrice: 11100 },
    ],
    deliveryFee: 6000,
    payment: {
      method: "Nequi / Daviplata",
      status: "POR_VERIFICAR",
      label: "Comprobante adjunto / Por verificar",
    },
  },
  {
    id: "ord-2",
    orderNumber: "#NX-1049",
    channel: "MOSTRADOR",
    status: "LISTA",
    createdAt: "Hace 26 min",
    timeReceived: "11:34 AM",
    customer: {
      name: "Consumidor Final",
      phone: "300 000 0000",
      deliveryType: "MESA",
      tableOrTakeoutInfo: "Mesa 4 • Salón Principal",
    },
    items: [
      { qty: 1, name: "Sándwich Roast Beef", category: "Platos", unitPrice: 12400 },
      { qty: 2, name: "Jugo Natural Maracuyá", category: "Bebidas", unitPrice: 3000 },
    ],
    deliveryFee: 0,
    payment: {
      method: "Efectivo",
      status: "PAGADO",
      label: "Efectivo Pagado en Caja",
    },
    cashierName: "Cajero 01",
  },
  {
    id: "ord-3",
    orderNumber: "#NX-1050",
    channel: "TIENDA_ONLINE",
    status: "CREADA",
    createdAt: "Hace 3 min",
    timeReceived: "11:57 AM",
    customer: {
      name: "Carlos Andrés Vélez",
      phone: "312 778 9900",
      address: "Calle 10 sur #34-89 Casa 12",
      addressNotes: "Portería 2, dejar con guardia",
      deliveryType: "EXPRESS",
    },
    items: [
      { qty: 1, name: "Combo Desayuno Gourmet", category: "Desayunos", unitPrice: 24900 },
      { qty: 1, name: "Torta de Chocolate 500g", category: "Pastelería", unitPrice: 30000 },
      { qty: 2, name: "Capuchino Vainilla", category: "Bebidas calientes", unitPrice: 7000 },
    ],
    deliveryFee: 7000,
    payment: {
      method: "Wompi / Tarjeta",
      status: "PAGADO",
      label: "Pasarela Aprobada",
    },
  },
  {
    id: "ord-4",
    orderNumber: "#NX-1047",
    channel: "WHATSAPP",
    status: "DESPACHADA",
    createdAt: "Hace 41 min",
    timeReceived: "11:19 AM",
    customer: {
      name: "Laura Gómez",
      phone: "315 890 1122",
      address: "Diag 75B #2A-120 Int 402",
      addressNotes: "Repartidor: Wilson R. (Moto 02)",
      deliveryType: "DOMICILIO_PROPIO",
    },
    items: [
      { qty: 1, name: "Pizza Personal 4 Quesos", category: "Comidas rápidas", unitPrice: 24200 },
      { qty: 1, name: "Cerveza Corona 355 ml", category: "Licores y cervezas", unitPrice: 7000 },
    ],
    deliveryFee: 5000,
    payment: {
      method: "Transferencia Bancolombia",
      status: "PAGADO",
      label: "Transferencia Bancolombia Verificada",
    },
  },
  {
    id: "ord-5",
    orderNumber: "#NX-1046",
    channel: "MOSTRADOR",
    status: "DESPACHADA",
    createdAt: "Hace 1 h",
    timeReceived: "11:00 AM",
    customer: {
      name: "Juan Diego Peña",
      phone: "318 456 7812",
      deliveryType: "PARA_LLEVAR",
      tableOrTakeoutInfo: "Para llevar • Mostrador",
    },
    items: [
      { qty: 1, name: "Desayuno Tradicional Paisa", category: "Desayunos", unitPrice: 12500 },
    ],
    deliveryFee: 0,
    payment: {
      method: "Tarjeta Débito",
      status: "PAGADO",
      label: "Datáfono Aprobado #F-4412",
    },
    cashierName: "Caja Principal",
  },
  {
    id: "ord-6",
    orderNumber: "#NX-1045",
    channel: "TIENDA_ONLINE",
    status: "EN_PREPARACION",
    createdAt: "Hace 52 min",
    timeReceived: "11:08 AM",
    customer: {
      name: "Sofía Restrepo",
      phone: "300 214 7890",
      frequent: true,
      address: "Circular 4 #73-10 Laureles",
      addressNotes: "Timbre blanco de la reja",
      deliveryType: "DOMICILIO_PROPIO",
    },
    items: [
      { qty: 2, name: "Bowl Acai Saludable", category: "Ensaladas y Bowls", unitPrice: 22000 },
      { qty: 1, name: "Jugo Verde Detox 500ml", category: "Bebidas naturales", unitPrice: 11000 },
    ],
    deliveryFee: 5000,
    payment: {
      method: "Tarjeta Crédito Wompi",
      status: "PAGADO",
      label: "Aprobado Wompi",
    },
  },
  {
    id: "ord-7",
    orderNumber: "#NX-1044",
    channel: "WHATSAPP",
    status: "CREADA",
    createdAt: "Hace 10 min",
    timeReceived: "11:50 AM",
    customer: {
      name: "Esteban Morales",
      phone: "311 320 4455",
      address: "Transversal 39 #70C-15",
      deliveryType: "DOMICILIO_PROPIO",
    },
    items: [
      { qty: 3, name: "Hamburguesa Artesanal Clásica", category: "Hamburguesas", unitPrice: 19500 },
      { qty: 3, name: "Papas Rústicas con Queso", category: "Acompañamientos", unitPrice: 6500 },
      { qty: 1, name: "Gaseosa Postobón 1.5 L", category: "Bebidas", unitPrice: 5500 },
    ],
    deliveryFee: 6000,
    payment: {
      method: "Contra entrega (Efectivo)",
      status: "PENDIENTE",
      label: "Paga con billete de $100.000",
    },
  },
  {
    id: "ord-8",
    orderNumber: "#NX-1043",
    channel: "MOSTRADOR",
    status: "EN_PREPARACION",
    createdAt: "Hace 35 min",
    timeReceived: "11:25 AM",
    customer: {
      name: "Alejandro Castro",
      phone: "320 654 3210",
      deliveryType: "MESA",
      tableOrTakeoutInfo: "Mesa 8 • Terraza",
    },
    items: [
      { qty: 2, name: "Club Colombia Dorada 330ml", category: "Cervezas", unitPrice: 5000 },
      { qty: 1, name: "Picada Típica Familiar", category: "Especiales", unitPrice: 48000 },
    ],
    deliveryFee: 0,
    payment: {
      method: "Cuenta abierta / En mesa",
      status: "PENDIENTE",
      label: "Cobro pendiente al cierre",
    },
  },
  {
    id: "ord-9",
    orderNumber: "#NX-1042",
    channel: "TIENDA_ONLINE",
    status: "LISTA",
    createdAt: "Hace 20 min",
    timeReceived: "11:40 AM",
    customer: {
      name: "Valentina Henao",
      phone: "314 987 6543",
      address: "Carrera 25 #3-45 Los Balsos",
      deliveryType: "EXPRESS",
    },
    items: [
      { qty: 1, name: "Cheesecake de Frutos Rojos", category: "Repostería", unitPrice: 14500 },
      { qty: 2, name: "Té Chai Especiado", category: "Bebidas calientes", unitPrice: 8500 },
    ],
    deliveryFee: 6500,
    payment: {
      method: "Nequi en línea",
      status: "PAGADO",
      label: "Confirmado automáticamente",
    },
  },
  {
    id: "ord-10",
    orderNumber: "#NX-1041",
    channel: "WHATSAPP",
    status: "CANCELADA",
    createdAt: "Hace 1 h 20 min",
    timeReceived: "10:40 AM",
    customer: {
      name: "Mateo Giraldo",
      phone: "301 555 4321",
      address: "Calle 50 #65-20",
      deliveryType: "DOMICILIO_PROPIO",
    },
    items: [
      { qty: 1, name: "Licor Aguardiente Antioqueño 750ml", category: "Licores", unitPrice: 45000 },
    ],
    deliveryFee: 5000,
    payment: {
      method: "Sin pago",
      status: "PENDIENTE",
      label: "Cancelada por cliente (demora)",
    },
  },
  {
    id: "ord-11",
    orderNumber: "#NX-1040",
    channel: "MOSTRADOR",
    status: "CANCELADA",
    createdAt: "Hace 2 h",
    timeReceived: "10:00 AM",
    customer: {
      name: "Cliente Casual",
      phone: "300 111 2233",
      deliveryType: "PARA_LLEVAR",
      tableOrTakeoutInfo: "Mostrador",
    },
    items: [
      { qty: 2, name: "Tinto Campesino", category: "Café", unitPrice: 2000 },
    ],
    deliveryFee: 0,
    payment: {
      method: "Efectivo",
      status: "PENDIENTE",
      label: "Error de digitación en caja",
    },
  },
  {
    id: "ord-12",
    orderNumber: "#NX-1039",
    channel: "WHATSAPP",
    status: "LISTA",
    createdAt: "Hace 15 min",
    timeReceived: "11:45 AM",
    customer: {
      name: "Felipe Cardona",
      phone: "317 222 8899",
      address: "Calle 33 #80-14 Laureles",
      deliveryType: "DOMICILIO_PROPIO",
    },
    items: [
      { qty: 2, name: "Alitas BBQ x 12", category: "Comidas rápidas", unitPrice: 26000 },
      { qty: 2, name: "Papas Francesas", category: "Acompañamientos", unitPrice: 5000 },
    ],
    deliveryFee: 5500,
    payment: {
      method: "Nequi",
      status: "PAGADO",
      label: "Comprobante verificado",
    },
  },
];

const STATUS_TABS: { id: OmnichannelOrderStatus | "TODAS"; label: string }[] = [
  { id: "TODAS", label: "Todas" },
  { id: "CREADA", label: "Creada" },
  { id: "EN_PREPARACION", label: "En preparación" },
  { id: "LISTA", label: "Lista para entrega" },
  { id: "DESPACHADA", label: "Despachada / Entregada" },
  { id: "CANCELADA", label: "Cancelada" },
];

function statusColor(status: OmnichannelOrderStatus) {
  switch (status) {
    case "CREADA":
      return "bg-slate-100 text-slate-700 border-slate-200";
    case "EN_PREPARACION":
      return "bg-sky-50 text-sky-700 border-sky-200";
    case "LISTA":
      return "bg-amber-50 text-amber-700 border-amber-200";
    case "DESPACHADA":
      return "bg-emerald-50 text-emerald-700 border-emerald-200";
    case "CANCELADA":
      return "bg-rose-50 text-rose-700 border-rose-200";
  }
}

function statusLabel(status: OmnichannelOrderStatus) {
  switch (status) {
    case "CREADA":
      return "Creada";
    case "EN_PREPARACION":
      return "En preparación";
    case "LISTA":
      return "Lista para entrega";
    case "DESPACHADA":
      return "Despachada";
    case "CANCELADA":
      return "Cancelada";
  }
}

function channelBadge(channel: OrderChannel) {
  switch (channel) {
    case "WHATSAPP":
      return {
        icon: MessageCircle,
        label: "WhatsApp",
        style: "bg-emerald-50 text-emerald-700 border-emerald-200",
        iconColor: "text-[#25D366]",
      };
    case "MOSTRADOR":
      return {
        icon: ShoppingBag,
        label: "Mostrador POS",
        style: "bg-sky-50 text-sky-700 border-sky-200",
        iconColor: "text-sky-600",
      };
    case "TIENDA_ONLINE":
      return {
        icon: ShoppingCart,
        label: "Tienda Online",
        style: "bg-purple-50 text-purple-700 border-purple-200",
        iconColor: "text-purple-600",
      };
  }
}

function calculateOrderTotal(order: MockOrder): number {
  const subtotal = order.items.reduce((acc, it) => acc + it.qty * it.unitPrice, 0);
  return subtotal + order.deliveryFee;
}

export function OrdersScreen() {
  const [orders, setOrders] = useState<MockOrder[]>(INITIAL_MOCK_ORDERS);
  const [selectedStatusTab, setSelectedStatusTab] = useState<OmnichannelOrderStatus | "TODAS">("TODAS");
  const [selectedChannel, setSelectedChannel] = useState<OrderChannel | "TODOS">("TODOS");
  const [searchQuery, setSearchQuery] = useState("");
  const [selectedOrder, setSelectedOrder] = useState<MockOrder | null>(null);

  // Counters for tabs
  const counts = useMemo(() => {
    const map: Record<string, number> = {
      TODAS: orders.length,
      CREADA: 0,
      EN_PREPARACION: 0,
      LISTA: 0,
      DESPACHADA: 0,
      CANCELADA: 0,
    };
    for (const o of orders) {
      map[o.status] = (map[o.status] || 0) + 1;
    }
    return map;
  }, [orders]);

  // Filtered orders
  const filteredOrders = useMemo(() => {
    return orders.filter((o) => {
      if (selectedStatusTab !== "TODAS" && o.status !== selectedStatusTab) {
        return false;
      }
      if (selectedChannel !== "TODOS" && o.channel !== selectedChannel) {
        return false;
      }
      if (searchQuery.trim()) {
        const q = searchQuery.toLowerCase().trim();
        const matchesNumber = o.orderNumber.toLowerCase().includes(q);
        const matchesCustomer = o.customer.name.toLowerCase().includes(q);
        const matchesPhone = o.customer.phone.toLowerCase().includes(q);
        if (!matchesNumber && !matchesCustomer && !matchesPhone) {
          return false;
        }
      }
      return true;
    });
  }, [orders, selectedStatusTab, selectedChannel, searchQuery]);

  // Advance state helper
  function advanceStatus(orderId: string) {
    setOrders((prev) =>
      prev.map((o) => {
        if (o.id !== orderId) return o;
        let nextStatus: OmnichannelOrderStatus = o.status;
        if (o.status === "CREADA") nextStatus = "EN_PREPARACION";
        else if (o.status === "EN_PREPARACION") nextStatus = "LISTA";
        else if (o.status === "LISTA") nextStatus = "DESPACHADA";
        const updated = { ...o, status: nextStatus };
        if (selectedOrder?.id === orderId) {
          setSelectedOrder(updated);
        }
        return updated;
      }),
    );
  }

  function cancelOrder(orderId: string) {
    setOrders((prev) =>
      prev.map((o) => {
        if (o.id !== orderId) return o;
        const updated: MockOrder = { ...o, status: "CANCELADA" };
        if (selectedOrder?.id === orderId) {
          setSelectedOrder(updated);
        }
        return updated;
      }),
    );
  }

  return (
    <div className="flex-1 space-y-6 overflow-y-auto p-4 lg:p-8">
      {/* 1. Header con métricas rápidas */}
      <div className="flex flex-col gap-4 md:flex-row md:items-center md:justify-between">
        <div className="flex items-center gap-3">
          <h1 className="font-headline text-2xl font-bold tracking-tight text-slate-900">
            Gestión de Órdenes
          </h1>
          <div className="inline-flex items-center gap-1.5 rounded-full border border-emerald-200/80 bg-emerald-50 px-3 py-1 text-xs font-semibold text-emerald-700 shadow-xs">
            <span className="size-2 animate-ping rounded-full bg-emerald-500" />
            <span>{orders.length} órdenes registradas</span>
          </div>
        </div>

        <div className="flex items-center gap-3 rounded-2xl border border-slate-200/70 bg-white px-4 py-2.5 text-xs text-slate-500 shadow-xs">
          <div className="flex items-center gap-1.5">
            <span className="text-slate-400">Total facturado hoy:</span>
            <span className="font-headline font-bold text-slate-900">$2.840.500 COP</span>
          </div>
          <span className="text-slate-200">|</span>
          <div className="flex items-center gap-1.5">
            <Clock className="size-3.5 text-teal-600" />
            <span className="text-slate-400">Tiempo prom. entrega:</span>
            <span className="font-semibold text-slate-800">18 min</span>
          </div>
        </div>
      </div>

      {/* 2. Tabs Horizontales por Estado */}
      <div className="flex items-center gap-2 overflow-x-auto pb-1 text-xs font-medium no-scrollbar">
        {STATUS_TABS.map((tab) => {
          const active = selectedStatusTab === tab.id;
          const count = counts[tab.id] ?? 0;
          return (
            <button
              key={tab.id}
              type="button"
              onClick={() => setSelectedStatusTab(tab.id)}
              className={cn(
                "flex items-center gap-2 rounded-xl px-4 py-2.5 font-semibold transition-all whitespace-nowrap shadow-xs",
                active
                  ? "bg-teal-700 text-white shadow-sm"
                  : "border border-slate-200/80 bg-white text-slate-700 hover:bg-slate-50",
              )}
            >
              <span>{tab.label}</span>
              <span
                className={cn(
                  "rounded-full px-2 py-0.5 text-[11px] font-bold",
                  active
                    ? "bg-white/20 text-white"
                    : "border border-slate-200 bg-slate-100 text-slate-700",
                )}
              >
                {count}
              </span>
            </button>
          );
        })}
      </div>

      {/* 3. Barra de Búsqueda y Chips de Canal */}
      <div className="flex flex-col items-center justify-between gap-4 rounded-2xl border border-slate-200/60 bg-white p-4 shadow-sm xl:flex-row">
        {/* Buscador */}
        <div className="relative w-full xl:w-96">
          <Search className="absolute top-1/2 left-3.5 size-4 -translate-y-1/2 text-slate-400" />
          <input
            type="text"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            placeholder="Buscar por N° orden, cliente o teléfono..."
            className="w-full rounded-xl border border-slate-200 bg-slate-50 py-2 pr-4 pl-10 text-xs text-slate-800 outline-none transition-all placeholder:text-slate-400 focus:border-teal-700 focus:bg-white focus:ring-1 focus:ring-teal-700"
          />
          {searchQuery && (
            <button
              type="button"
              onClick={() => setSearchQuery("")}
              className="absolute top-1/2 right-3 -translate-y-1/2 text-slate-400 hover:text-slate-600"
            >
              <X className="size-3.5" />
            </button>
          )}
        </div>

        {/* Chips de Canal */}
        <div className="flex w-full items-center gap-2 overflow-x-auto xl:w-auto">
          <button
            type="button"
            onClick={() => setSelectedChannel("TODOS")}
            className={cn(
              "rounded-xl px-3.5 py-1.5 text-xs font-semibold whitespace-nowrap transition-colors",
              selectedChannel === "TODOS"
                ? "bg-slate-900 text-white shadow-xs"
                : "border border-slate-200/80 bg-white text-slate-600 hover:bg-slate-50",
            )}
          >
            Todos los canales
          </button>

          <button
            type="button"
            onClick={() => setSelectedChannel("MOSTRADOR")}
            className={cn(
              "flex items-center gap-1.5 rounded-xl border px-3.5 py-1.5 text-xs font-semibold whitespace-nowrap transition-colors",
              selectedChannel === "MOSTRADOR"
                ? "border-sky-300 bg-sky-600 text-white shadow-xs"
                : "border-sky-200/70 bg-sky-50 text-sky-800 hover:bg-sky-100",
            )}
          >
            <ShoppingBag className={cn("size-3.5", selectedChannel === "MOSTRADOR" ? "text-white" : "text-sky-600")} />
            <span>Mostrador POS</span>
          </button>

          <button
            type="button"
            onClick={() => setSelectedChannel("WHATSAPP")}
            className={cn(
              "flex items-center gap-1.5 rounded-xl border px-3.5 py-1.5 text-xs font-semibold whitespace-nowrap transition-colors",
              selectedChannel === "WHATSAPP"
                ? "border-emerald-400 bg-[#25D366] text-white shadow-xs"
                : "border-emerald-200/70 bg-emerald-50 text-emerald-800 hover:bg-emerald-100",
            )}
          >
            <MessageCircle className={cn("size-3.5", selectedChannel === "WHATSAPP" ? "text-white" : "text-[#25D366]")} />
            <span>WhatsApp</span>
          </button>

          <button
            type="button"
            onClick={() => setSelectedChannel("TIENDA_ONLINE")}
            className={cn(
              "flex items-center gap-1.5 rounded-xl border px-3.5 py-1.5 text-xs font-semibold whitespace-nowrap transition-colors",
              selectedChannel === "TIENDA_ONLINE"
                ? "border-purple-300 bg-purple-700 text-white shadow-xs"
                : "border-purple-200/70 bg-purple-50 text-purple-800 hover:bg-purple-100",
            )}
          >
            <ShoppingCart className={cn("size-3.5", selectedChannel === "TIENDA_ONLINE" ? "text-white" : "text-purple-600")} />
            <span>Tienda Online</span>
          </button>
        </div>
      </div>

      {/* 4. Grilla de Tarjetas de Órdenes */}
      {filteredOrders.length === 0 ? (
        <div className="flex flex-col items-center justify-center rounded-2xl border border-dashed border-slate-200 bg-white p-12 text-center">
          <AlertCircle className="size-10 text-slate-300" />
          <h3 className="mt-3 text-sm font-bold text-slate-800">No se encontraron órdenes</h3>
          <p className="mt-1 text-xs text-slate-500">
            Probá ajustando los filtros de estado, canal o el texto de búsqueda.
          </p>
        </div>
      ) : (
        <div className="grid grid-cols-1 gap-5 md:grid-cols-2 xl:grid-cols-3">
          {filteredOrders.map((order) => {
            const ch = channelBadge(order.channel);
            const ChIcon = ch.icon;
            const total = calculateOrderTotal(order);
            const totalItemsCount = order.items.reduce((acc, it) => acc + it.qty, 0);
            const isSelected = selectedOrder?.id === order.id;

            return (
              <article
                key={order.id}
                onClick={() => setSelectedOrder(order)}
                className={cn(
                  "relative flex cursor-pointer flex-col justify-between rounded-2xl bg-white p-5 transition-all shadow-xs hover:shadow-md",
                  isSelected
                    ? "border-2 border-teal-600 shadow-md ring-4 ring-teal-500/10"
                    : "border border-slate-200/80 hover:border-slate-300",
                )}
              >
                {isSelected && <div className="absolute top-0 right-0 left-0 h-1 bg-teal-600" />}

                <div>
                  {/* Header de la tarjeta */}
                  <div className="mb-3 flex items-center justify-between gap-2">
                    <div className="flex items-center gap-2">
                      <span className="font-headline text-base font-bold text-slate-900">
                        {order.orderNumber}
                      </span>
                      <span
                        className={cn(
                          "inline-flex items-center gap-1 rounded-full border px-2 py-0.5 text-[11px] font-semibold",
                          ch.style,
                        )}
                      >
                        <ChIcon className={cn("size-3", ch.iconColor)} />
                        <span>{ch.label}</span>
                      </span>
                    </div>

                    {/* Badge de estado grande */}
                    <span
                      className={cn(
                        "rounded-full border px-2.5 py-0.5 text-[11px] font-semibold",
                        statusColor(order.status),
                      )}
                    >
                      {statusLabel(order.status)}
                    </span>
                  </div>

                  {/* Datos del Cliente */}
                  <div className="mb-3 space-y-1 border-b border-slate-100 pb-3">
                    <div className="flex items-center justify-between">
                      <h2 className="font-headline text-sm font-bold text-slate-900">
                        {order.customer.name}
                      </h2>
                      <span className="text-[11px] text-slate-400">{order.createdAt}</span>
                    </div>

                    <div className="flex items-center gap-1.5 text-xs text-slate-500">
                      <Phone className="size-3.5 text-slate-400" />
                      <span>{order.customer.phone}</span>
                    </div>

                    {order.customer.address ? (
                      <div className="flex items-center gap-1.5 text-xs text-slate-600 truncate">
                        <MapPin className="size-3.5 shrink-0 text-slate-400" />
                        <span className="truncate">{order.customer.address}</span>
                      </div>
                    ) : order.customer.tableOrTakeoutInfo ? (
                      <div className="flex items-center gap-1.5 text-xs text-slate-600">
                        {order.customer.deliveryType === "MESA" ? (
                          <UtensilsCrossed className="size-3.5 text-slate-400" />
                        ) : (
                          <Store className="size-3.5 text-slate-400" />
                        )}
                        <span className="font-medium text-slate-700">
                          {order.customer.tableOrTakeoutInfo}
                        </span>
                      </div>
                    ) : null}
                  </div>

                  {/* Resumen de Ítems (2-3 líneas + "+N otros") */}
                  <div className="mb-4 space-y-1 text-xs text-slate-600">
                    <p className="font-medium text-slate-800">
                      {totalItemsCount} {totalItemsCount === 1 ? "ítem solicitado:" : "ítems solicitados:"}
                    </p>
                    <ul className="space-y-0.5 border-l border-slate-200 pl-2 text-[11px] text-slate-500">
                      {order.items.slice(0, 3).map((item, idx) => (
                        <li key={idx} className="truncate">
                          • {item.qty}x {item.name}
                        </li>
                      ))}
                      {order.items.length > 3 && (
                        <li className="font-medium text-teal-700">
                          + {order.items.length - 3} {order.items.length - 3 === 1 ? "otro ítem" : "otros ítems"}...
                        </li>
                      )}
                    </ul>
                  </div>
                </div>

                {/* Footer: Total COP y Estado de Pago */}
                <div className="flex items-center justify-between border-t border-slate-100 pt-3">
                  <div>
                    <span className="block text-[10px] font-bold tracking-wider text-slate-400 uppercase">
                      Total COP
                    </span>
                    <span className="font-headline text-base font-bold text-slate-900">
                      {formatCOP(total)}
                    </span>
                  </div>

                  <span
                    className={cn(
                      "flex items-center gap-1 rounded-lg border px-2.5 py-1 text-[11px] font-medium",
                      order.payment.status === "PAGADO"
                        ? "border-emerald-200 bg-emerald-50 text-emerald-700"
                        : order.payment.status === "POR_VERIFICAR"
                          ? "border-amber-200 bg-amber-50 text-amber-700"
                          : "border-slate-200 bg-slate-50 text-slate-600",
                    )}
                  >
                    {order.payment.status === "PAGADO" && <CheckCircle2 className="size-3 text-emerald-600" />}
                    {order.payment.status === "POR_VERIFICAR" && <Clock className="size-3 text-amber-600" />}
                    <span>{order.payment.label}</span>
                  </span>
                </div>
              </article>
            );
          })}
        </div>
      )}

      {/* 5. Modal de Detalle Completo de la Orden */}
      <Dialog
        open={Boolean(selectedOrder)}
        onOpenChange={(open) => {
          if (!open) setSelectedOrder(null);
        }}
      >
        {selectedOrder && (
          <DialogContent className="max-h-[90vh] w-full max-w-xl overflow-y-auto rounded-3xl p-6 sm:p-8">
            <div className="flex items-start justify-between border-b border-slate-100 pb-4">
              <div>
                <div className="mb-1 flex items-center gap-2">
                  <DialogTitle className="font-headline text-xl font-bold tracking-tight text-slate-900">
                    {selectedOrder.orderNumber}
                  </DialogTitle>
                  {(() => {
                    const ch = channelBadge(selectedOrder.channel);
                    const ChIcon = ch.icon;
                    return (
                      <span
                        className={cn(
                          "inline-flex items-center gap-1 rounded-full border px-2.5 py-0.5 text-xs font-semibold",
                          ch.style,
                        )}
                      >
                        <ChIcon className={cn("size-3.5", ch.iconColor)} />
                        <span>{ch.label}</span>
                      </span>
                    );
                  })()}
                </div>
                <div className="flex items-center gap-2">
                  <span
                    className={cn(
                      "rounded-full border px-2.5 py-0.5 text-xs font-semibold",
                      statusColor(selectedOrder.status),
                    )}
                  >
                    {statusLabel(selectedOrder.status)}
                  </span>
                  <span className="text-xs text-slate-400">
                    Recibida a las {selectedOrder.timeReceived} ({selectedOrder.createdAt})
                  </span>
                </div>
              </div>
            </div>

            {/* Línea de Tiempo de Estados */}
            <div className="mt-4 rounded-2xl border border-slate-200/80 bg-slate-50 p-4">
              <span className="block text-[11px] font-bold tracking-wider text-slate-400 uppercase">
                Línea de tiempo del pedido
              </span>
              <div className="mt-3 flex items-center justify-between text-xs">
                {(["CREADA", "EN_PREPARACION", "LISTA", "DESPACHADA"] as OmnichannelOrderStatus[]).map(
                  (step, idx, arr) => {
                    const stepOrder = ["CREADA", "EN_PREPARACION", "LISTA", "DESPACHADA"];
                    const currentIdx = stepOrder.indexOf(selectedOrder.status);
                    const stepIdx = stepOrder.indexOf(step);
                    const isDone = selectedOrder.status !== "CANCELADA" && currentIdx >= stepIdx;
                    const isCurrent = selectedOrder.status === step;

                    return (
                      <div key={step} className="flex flex-1 items-center">
                        <div className="flex flex-col items-center">
                          <div
                            className={cn(
                              "flex size-6 items-center justify-center rounded-full text-[11px] font-bold",
                              isDone
                                ? "bg-teal-700 text-white"
                                : "border border-slate-300 bg-white text-slate-400",
                            )}
                          >
                            {isDone ? <CheckCircle2 className="size-4" /> : idx + 1}
                          </div>
                          <span
                            className={cn(
                              "mt-1 text-[10px] text-center font-medium",
                              isCurrent ? "font-bold text-teal-800" : isDone ? "text-slate-700" : "text-slate-400",
                            )}
                          >
                            {statusLabel(step)}
                          </span>
                        </div>
                        {idx < arr.length - 1 && (
                          <div
                            className={cn(
                              "mx-1.5 h-0.5 flex-1",
                              selectedOrder.status !== "CANCELADA" && currentIdx > stepIdx
                                ? "bg-teal-600"
                                : "bg-slate-200",
                            )}
                          />
                        )}
                      </div>
                    );
                  },
                )}
              </div>
              {selectedOrder.status === "CANCELADA" && (
                <div className="mt-3 flex items-center gap-2 rounded-xl bg-rose-50 p-2.5 text-xs font-semibold text-rose-700">
                  <XCircle className="size-4 shrink-0" />
                  <span>Esta orden fue cancelada.</span>
                </div>
              )}
            </div>

            {/* Datos del Cliente y Acción de WhatsApp */}
            <div className="mt-4 rounded-2xl border border-slate-200/80 bg-slate-50 p-4">
              <div className="flex items-start justify-between">
                <div>
                  <p className="text-[10px] font-bold tracking-wider text-slate-400 uppercase">
                    Cliente
                  </p>
                  <h3 className="font-headline text-sm font-bold text-slate-900">
                    {selectedOrder.customer.name}
                  </h3>
                  <p className="text-xs font-medium text-slate-600">
                    Teléfono: {selectedOrder.customer.phone}
                  </p>
                </div>
                {selectedOrder.customer.frequent && (
                  <span className="rounded-md border border-teal-200 bg-teal-50 px-2 py-0.5 text-[10px] font-bold text-teal-800">
                    Cliente Frecuente
                  </span>
                )}
              </div>

              {selectedOrder.customer.address && (
                <div className="mt-3 flex items-start gap-2 border-t border-slate-200/60 pt-2 text-xs text-slate-600">
                  <MapPin className="mt-0.5 size-4 shrink-0 text-slate-400" />
                  <div>
                    <span className="font-medium text-slate-800">
                      {selectedOrder.customer.address}
                    </span>
                    {selectedOrder.customer.addressNotes && (
                      <p className="text-[11px] text-slate-500">
                        Indicación: {selectedOrder.customer.addressNotes}
                      </p>
                    )}
                  </div>
                </div>
              )}

              {selectedOrder.customer.tableOrTakeoutInfo && (
                <div className="mt-3 flex items-center gap-2 border-t border-slate-200/60 pt-2 text-xs text-slate-700">
                  <Store className="size-4 text-slate-400" />
                  <span className="font-medium">{selectedOrder.customer.tableOrTakeoutInfo}</span>
                </div>
              )}

              {/* Botón WhatsApp */}
              {selectedOrder.customer.phone && selectedOrder.customer.phone !== "300 000 0000" && (
                <a
                  href={`https://wa.me/57${selectedOrder.customer.phone.replace(/\D/g, "")}`}
                  target="_blank"
                  rel="noopener noreferrer"
                  className="mt-3 flex w-full items-center justify-center gap-2 rounded-xl bg-[#25D366] px-3 py-2.5 text-xs font-semibold text-white shadow-sm transition-all hover:bg-[#20ba59]"
                >
                  <MessageCircle className="size-4" />
                  <span>Chatear por WhatsApp con el cliente</span>
                </a>
              )}
            </div>

            {/* Desglose de Ítems */}
            <div className="mt-4 space-y-2">
              <div className="flex items-center justify-between">
                <h4 className="text-xs font-bold tracking-wider text-slate-500 uppercase">
                  Ítems de la orden ({selectedOrder.items.length})
                </h4>
              </div>
              <div className="max-h-56 space-y-2 overflow-y-auto pr-1">
                {selectedOrder.items.map((item, idx) => (
                  <div
                    key={idx}
                    className="flex items-center justify-between rounded-xl border border-slate-100 bg-slate-50/70 p-2.5 text-xs"
                  >
                    <div className="flex items-center gap-2.5">
                      <div className="flex size-7 items-center justify-center rounded-lg border border-teal-100 bg-teal-50 text-xs font-bold text-teal-700">
                        {item.qty}x
                      </div>
                      <div>
                        <p className="font-semibold text-slate-800">{item.name}</p>
                        <p className="text-[10px] text-slate-400">
                          {item.category} • {formatCOP(item.unitPrice)} c/u
                        </p>
                      </div>
                    </div>
                    <span className="font-bold text-slate-900">
                      {formatCOP(item.qty * item.unitPrice)}
                    </span>
                  </div>
                ))}
              </div>
            </div>

            {/* Totales y Pago */}
            <div className="mt-4 space-y-2 border-t border-slate-100 pt-3">
              <div className="flex justify-between text-xs text-slate-500">
                <span>Subtotal productos</span>
                <span>
                  {formatCOP(
                    selectedOrder.items.reduce((acc, it) => acc + it.qty * it.unitPrice, 0),
                  )}
                </span>
              </div>
              {selectedOrder.deliveryFee > 0 && (
                <div className="flex justify-between text-xs text-slate-500">
                  <span className="flex items-center gap-1">
                    <Truck className="size-3.5 text-slate-400" />
                    <span>Tarifa domicilio</span>
                  </span>
                  <span>{formatCOP(selectedOrder.deliveryFee)}</span>
                </div>
              )}
              <div className="flex justify-between border-t border-slate-100 pt-2 font-headline text-base font-bold text-slate-900">
                <span>Total a cobrar</span>
                <span className="text-lg text-teal-700">
                  {formatCOP(calculateOrderTotal(selectedOrder))}
                </span>
              </div>

              {/* Método de pago */}
              <div className="mt-2 flex items-center justify-between rounded-xl border border-amber-200/80 bg-amber-50/70 p-3 text-xs">
                <div>
                  <span className="block text-[11px] font-bold text-slate-800">
                    {selectedOrder.payment.method}
                  </span>
                  <span className="text-[10px] text-amber-800">{selectedOrder.payment.label}</span>
                </div>
                <span
                  className={cn(
                    "rounded-full px-2 py-0.5 text-[10px] font-bold",
                    selectedOrder.payment.status === "PAGADO"
                      ? "bg-emerald-100 text-emerald-800"
                      : "bg-amber-100 text-amber-800",
                  )}
                >
                  {selectedOrder.payment.status}
                </span>
              </div>
            </div>

            {/* Acciones Operativas */}
            <div className="mt-5 space-y-2 border-t border-slate-100 pt-4">
              {selectedOrder.status !== "DESPACHADA" && selectedOrder.status !== "CANCELADA" && (
                <button
                  type="button"
                  onClick={() => advanceStatus(selectedOrder.id)}
                  className="flex w-full items-center justify-center gap-2 rounded-xl bg-teal-700 py-3 text-xs font-bold text-white shadow-sm transition-all hover:bg-teal-800 active:scale-98"
                >
                  <CheckCircle2 className="size-4" />
                  <span>
                    {selectedOrder.status === "CREADA" && "Pasar a En preparación"}
                    {selectedOrder.status === "EN_PREPARACION" && "Marcar como Lista para entrega"}
                    {selectedOrder.status === "LISTA" && "Marcar como Despachada / Entregada"}
                  </span>
                </button>
              )}

              {selectedOrder.status !== "CANCELADA" && selectedOrder.status !== "DESPACHADA" && (
                <button
                  type="button"
                  onClick={() => cancelOrder(selectedOrder.id)}
                  className="flex w-full items-center justify-center gap-1.5 rounded-xl py-2 text-xs font-semibold text-rose-600 transition-colors hover:bg-rose-50 hover:text-rose-700"
                >
                  <XCircle className="size-4" />
                  <span>Cancelar orden</span>
                </button>
              )}
            </div>
          </DialogContent>
        )}
      </Dialog>
    </div>
  );
}
