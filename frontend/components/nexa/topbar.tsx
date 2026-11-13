import { Bell } from "lucide-react";

import { BranchSwitcher, UserMenu } from "@/components/auth/user-menu";

/** Barra superior del layout de administracion. */
export function Topbar({ title }: { title: string }) {
  return (
    <header className="sticky top-0 z-20 flex h-16 items-center justify-between border-b border-slate-200/60 bg-white/90 px-6 backdrop-blur lg:px-10">
      <nav className="flex items-center gap-2 text-sm">
        <span className="hidden text-slate-400 sm:inline">Panel Administrativo</span>
        <span className="hidden text-slate-300 sm:inline">/</span>
        <h1 className="font-semibold text-slate-900">{title}</h1>
      </nav>
      <div className="flex items-center gap-4">
        <BranchSwitcher className="hidden sm:flex" />
        <button aria-label="Notificaciones" className="relative text-slate-500 hover:text-slate-700">
          <Bell className="size-5" />
          <span className="absolute -top-1.5 -right-1.5 flex size-4 items-center justify-center rounded-full bg-rose-500 text-[10px] font-bold text-white">
            3
          </span>
        </button>
        <div className="border-l border-slate-200 pl-4">
          <UserMenu dark />
        </div>
      </div>
    </header>
  );
}
