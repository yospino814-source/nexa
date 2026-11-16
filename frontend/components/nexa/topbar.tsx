import { BranchSwitcher, UserMenu } from "@/components/auth/user-menu";

/** Barra superior de administración: 64 px, translúcida, con la ruta y el usuario. */
export function Topbar({ title }: { title: string }) {
  return (
    <header className="sticky top-0 z-20 flex h-16 items-center justify-between gap-4 border-b border-slate-200 bg-white/86 px-6 backdrop-blur-md lg:px-7">
      <nav aria-label="Ruta" className="flex min-w-0 items-center gap-2 text-sm whitespace-nowrap">
        <span className="hidden text-slate-500 sm:inline">Panel Administrativo</span>
        <span aria-hidden className="hidden text-slate-300 sm:inline">
          /
        </span>
        <h1 className="truncate font-semibold text-slate-900">{title}</h1>
      </nav>
      <div className="flex items-center gap-2.5">
        <BranchSwitcher className="hidden sm:flex" />
        <UserMenu />
      </div>
    </header>
  );
}
