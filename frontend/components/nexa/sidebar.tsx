"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import {
  BarChart3,
  Calculator,
  LayoutDashboard,
  LayoutGrid,
  LogOut,
  Package,
  Settings,
  ShoppingCart,
  Sparkles,
  TrendingUp,
  Users,
  Wallet,
  type LucideIcon,
} from "lucide-react";

import { BrandMark } from "@/components/nexa/brand";
import { canOpen } from "@/lib/route-permissions";
import { homeFor, useSession } from "@/lib/stores/session";
import { cn } from "@/lib/utils";

type NavItem = { href: string; label: string; icon: LucideIcon; match?: string[] };

const navItems: NavItem[] = [
  { href: "/panel", label: "Panel principal", icon: LayoutDashboard },
  { href: "/pos", label: "POS mostrador", icon: Calculator, match: ["/pos", "/caja"] },
  { href: "/inventario", label: "Inventario", icon: Package, match: ["/inventario", "/recetas"] },
  { href: "/compras", label: "Compras", icon: ShoppingCart },
  { href: "/ventas", label: "Reportes de ventas", icon: TrendingUp },
  { href: "/ordenes", label: "Ventas y órdenes", icon: BarChart3, match: ["/ordenes", "/gastos"] },
  { href: "/clientes", label: "Clientes", icon: Users },
  { href: "/cartera", label: "Cartera", icon: Wallet },
  { href: "/marketing/promociones", label: "Marketing", icon: Sparkles, match: ["/marketing"] },
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

/** Sidebar del layout de administracion: 72px, oscuro, solo iconos con tooltip. */
export function Sidebar() {
  const pathname = usePathname();
  const router = useRouter();
  const { branches, branchId, can, signOut } = useSession();
  const currentBranch = branches.find((b) => b.id === branchId) ?? branches[0];
  const role = currentBranch?.role ?? null;

  const isActive = (item: NavItem) =>
    (item.match ?? [item.href]).some((prefix) => pathname === prefix || pathname.startsWith(`${prefix}/`));

  const visibleNavItems = role === null ? [] : navItems.filter((item) => canOpen(item.href, can));
  const showModules = process.env.NODE_ENV !== "production";

  return (
    <aside className="sticky top-0 hidden h-screen w-[72px] shrink-0 flex-col items-center bg-sidebar py-4 md:flex">
      <Link href={homeFor(role)} aria-label="Inicio">
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
        <button
          type="button"
          aria-label="Cerrar sesión"
          onClick={async () => {
            await signOut();
            router.replace("/login");
          }}
          className="group relative flex size-12 items-center justify-center rounded-2xl text-sidebar-foreground transition-colors hover:bg-sidebar-accent/60 hover:text-sidebar-accent-foreground"
        >
          <LogOut className="size-5" />
          <span className="pointer-events-none absolute left-full z-50 ml-3 rounded-lg bg-slate-800 px-2.5 py-1 text-xs font-medium whitespace-nowrap text-white opacity-0 shadow-lg transition-opacity group-hover:opacity-100">
            Cerrar sesión
          </span>
        </button>
      </div>
    </aside>
  );
}
