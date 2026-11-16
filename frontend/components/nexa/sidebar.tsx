"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import {
  BarChart3,
  Box,
  Calculator,
  LayoutDashboard,
  LayoutGrid,
  LogOut,
  MessageCircle,
  Settings,
  ShoppingCart,
  Sparkles,
  Users,
  Wallet,
  type LucideIcon,
} from "lucide-react";

import { BrandMark } from "@/components/nexa/brand";
import { canOpen } from "@/lib/route-permissions";
import { homeFor, initials, useSession } from "@/lib/stores/session";
import { cn } from "@/lib/utils";

type NavItem = { href: string; label: string; title: string; icon: LucideIcon; match?: string[] };

const navItems: NavItem[] = [
  { href: "/panel", label: "Panel", title: "Panel principal", icon: LayoutDashboard },
  { href: "/pos", label: "POS", title: "POS y caja", icon: Calculator, match: ["/pos", "/caja"] },
  { href: "/pedidos", label: "Pedidos", title: "Pedidos por WhatsApp", icon: MessageCircle },
  { href: "/inventario", label: "Inventario", title: "Inventario", icon: Box, match: ["/inventario", "/recetas"] },
  { href: "/compras", label: "Compras", title: "Compras", icon: ShoppingCart },
  {
    href: "/ordenes",
    label: "Ventas",
    title: "Ventas, reportes y gastos",
    icon: BarChart3,
    match: ["/ordenes", "/ventas", "/gastos"],
  },
  { href: "/clientes", label: "Clientes", title: "Clientes", icon: Users },
  { href: "/cartera", label: "Cartera", title: "Cartera", icon: Wallet },
  { href: "/marketing/promociones", label: "Marketing", title: "Promociones y cupones", icon: Sparkles, match: ["/marketing"] },
  { href: "/configuracion/negocio", label: "Ajustes", title: "Configuración", icon: Settings, match: ["/configuracion"] },
];

function NavLink({ item, active }: { item: NavItem; active: boolean }) {
  const Icon = item.icon;
  return (
    <Link
      href={item.href}
      title={item.title}
      aria-current={active ? "page" : undefined}
      className={cn(
        "relative flex h-[60px] w-[72px] flex-col items-center justify-center gap-1.5 rounded-xl font-medium transition-[transform,background-color,color] duration-150 active:scale-95",
        active ? "bg-white/8 text-white" : "text-slate-400 hover:bg-white/6 hover:text-white",
      )}
    >
      {active && (
        <span
          aria-hidden
          className="absolute top-3.5 bottom-3.5 -left-1.5 w-1 rounded bg-linear-to-b from-grad-pink to-grad-violet"
        />
      )}
      <Icon className="size-[22px]" strokeWidth={1.75} aria-hidden />
      <span className="text-[11px] leading-none">{item.label}</span>
    </Link>
  );
}

/** Menú lateral de administración: 84 px, carbón, ícono de 22 px con su nombre. */
export function Sidebar() {
  const pathname = usePathname();
  const router = useRouter();
  const { branches, branchId, can, signOut, name } = useSession();
  const currentBranch = branches.find((b) => b.id === branchId) ?? branches[0];
  const role = currentBranch?.role ?? null;

  const isActive = (item: NavItem) =>
    (item.match ?? [item.href]).some((prefix) => pathname === prefix || pathname.startsWith(`${prefix}/`));

  const visibleNavItems = role === null ? [] : navItems.filter((item) => canOpen(item.href, can));
  const showModules = process.env.NODE_ENV !== "production";

  return (
    <aside className="sticky top-0 hidden h-screen w-[84px] shrink-0 flex-col items-center gap-[18px] overflow-y-auto bg-carbon py-4 md:flex">
      <Link href={homeFor(role)} aria-label="Inicio" className="shrink-0">
        <BrandMark size={40} />
      </Link>
      <nav aria-label="Menú principal" className="flex flex-col gap-1">
        {visibleNavItems.map((item) => (
          <NavLink key={item.href} item={item} active={isActive(item)} />
        ))}
        {showModules && (
          <NavLink
            item={{ href: "/modulos", label: "Mapa", title: "Mapa de pantallas", icon: LayoutGrid }}
            active={pathname === "/modulos"}
          />
        )}
      </nav>
      <div className="mt-auto flex flex-col items-center gap-2.5">
        <span
          title={name ?? undefined}
          className="flex size-10 items-center justify-center rounded-full border-2 border-carbon-soft bg-indigo-600 text-[13px] font-semibold text-white"
        >
          {initials(name)}
        </span>
        <button
          type="button"
          onClick={async () => {
            await signOut();
            router.replace("/login");
          }}
          className="flex flex-col items-center gap-1 rounded-[10px] px-2 py-1.5 text-[11px] font-medium text-slate-400 transition-colors hover:bg-white/6 hover:text-white"
        >
          <LogOut className="size-5" strokeWidth={1.75} aria-hidden />
          Salir
        </button>
      </div>
    </aside>
  );
}
