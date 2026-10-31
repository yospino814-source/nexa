import { SessionGate } from "@/components/auth/session-gate";
import { OperationTopbar } from "@/components/nexa/operation-topbar";

/** Layout de operacion: pantalla completa, barra minima de 56px, sin sidebar. */
export default function OperationLayout({ children }: { children: React.ReactNode }) {
  return (
    <SessionGate>
      <div className="flex h-screen flex-col overflow-hidden bg-slate-50 text-slate-900">
        <OperationTopbar />
        {children}
      </div>
    </SessionGate>
  );
}
