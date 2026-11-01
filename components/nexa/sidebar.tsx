"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import {
  BarChart3,
  Calculator,
  LayoutDashboard,
  LayoutGrid,
  LogOut,
  Package,
  Settings,
  ShoppingCart,
  Truck,
  Users,
  UtensilsCrossed,
  type LucideIcon,
} from "lucide-react";

import { BrandMark } from "@/components/nexa/brand";
import { useSession } from "@/lib/stores/session";
import type { Role } from "@/lib/api";
import { cn } from "@/lib/utils";

type NavItem = { href: string; label: string; icon: LucideIcon; match?: string[] };

const navItems: NavItem[] = [
  { href: "/panel", label: "Panel principal", icon: LayoutDashboard },
  { href: "/pos", label: "POS mostrador", icon: Calculator, match: ["/pos", "/caja"] },
  { href: "/inventario", label: "Inventario", icon: Package, match: ["/inventario", "/recetas"] },
  { href: "/compras", label: "Compras", icon: ShoppingCart },
  { href: "/ventas", label: "Ventas y reportes", icon: BarChart3, match: ["/ventas", "/ordenes", "/gastos"] },
  { href: "/clientes", label: "Clientes", icon: Users },
  { href: "/mesas", label: "Restaurante", icon: UtensilsCrossed, match: ["/mesas", "/comanda", "/cocina"] },
  { href: "/rutas", label: "Rutas y mayorista", icon: Truck, match: ["/rutas", "/mayorista", "/repartidor"] },
  { href: "/configuracion/negocio", label: "Configuración", icon: Settings, match: ["/configuracion"] },
];

function NavLink({ item, active }: { item: NavItem; active: boolean }) {
  const Icon = item.icon;
  return (
    <Link
      href={item.href}
      aria-label={item.label}
      className={cn(
        "group relative flex size-12 items-center justify-center rounded-2xl transition-colors",
        active
          ? "bg-sidebar-accent text-sidebar-primary"
          : "text-sidebar-foreground hover:bg-sidebar-accent/60 hover:text-sidebar-accent-foreground",
      )}
    >
      {active && <span className="absolute -left-3 h-6 w-1 rounded-r-full bg-sidebar-primary" />}
      <Icon className="size-5" />
      <span className="pointer-events-none absolute left-full z-50 ml-3 rounded-lg bg-slate-800 px-2.5 py-1 text-xs font-medium whitespace-nowrap text-white opacity-0 shadow-lg transition-opacity group-hover:opacity-100">
        {item.label}
      </span>
    </Link>
  );
}

function isAllowedForRole(href: string, userRole: Role): boolean {
  if (userRole === "DUENO" || userRole === "ADMIN_CAJA") return true;
  if (userRole === "CAJERO") return ["/pos", "/panel", "/clientes"].includes(href);
  if (userRole === "MESERO") return href === "/mesas";
  if (userRole === "COCINA") return href === "/mesas";
  if (userRole === "SUPERVISOR")
    return ["/panel", "/pos", "/inventario", "/ventas", "/clientes", "/mesas"].includes(href);
  if (userRole === "BODEGA") return ["/inventario", "/compras"].includes(href);
  if (userRole === "REPARTIDOR") return href === "/rutas";
  return true;
}

/** Sidebar del layout de administracion: 72px, oscuro, solo iconos con tooltip. */
export function Sidebar() {
  const pathname = usePathname();
  const { branches, branchId } = useSession();
  const currentBranch = branches.find((b) => b.id === branchId) ?? branches[0];
  const role = currentBranch?.role ?? "DUENO";

  const isActive = (item: NavItem) =>
    (item.match ?? [item.href]).some((prefix) => pathname === prefix || pathname.startsWith(`${prefix}/`));

  const visibleNavItems = navItems.filter((item) => isAllowedForRole(item.href, role));
  const showModules = process.env.NODE_ENV !== "production";

  return (
    <aside className="sticky top-0 hidden h-screen w-[72px] shrink-0 flex-col items-center bg-sidebar py-4 md:flex">
      <Link href="/panel" aria-label="Inicio">
        <BrandMark className="[&>div]:size-11 [&>div]:rounded-full" />
      </Link>
      <nav className="mt-8 flex flex-1 flex-col items-center gap-2">
        {visibleNavItems.map((item) => (
          <NavLink key={item.href} item={item} active={isActive(item)} />
        ))}
      </nav>
      <div className="flex flex-col items-center gap-2 border-t border-sidebar-border pt-4">
        {showModules && (
          <NavLink item={{ href: "/modulos", label: "Mapa de pantallas", icon: LayoutGrid }} active={pathname === "/modulos"} />
        )}
        <NavLink item={{ href: "/login", label: "Cerrar sesión", icon: LogOut }} active={false} />
      </div>
    </aside>
  );
}
