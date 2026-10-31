import { SessionGate } from "@/components/auth/session-gate";
import { Sidebar } from "@/components/nexa/sidebar";

/** Layout de administracion: sidebar de 72px + contenido (cada pagina pone su Topbar). */
export default function AdminLayout({ children }: { children: React.ReactNode }) {
  return (
    <SessionGate>
      <div className="flex min-h-screen w-full bg-slate-50 text-slate-900">
        <Sidebar />
        <main className="min-w-0 flex-1">{children}</main>
      </div>
    </SessionGate>
  );
}
