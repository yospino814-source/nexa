import { BrandMark } from "@/components/nexa/brand";

/** Marco de las pantallas publicas de acceso: marca arriba y tarjeta centrada. */
export function AuthShell({
  title,
  subtitle,
  children,
}: {
  title: string;
  subtitle?: string;
  children: React.ReactNode;
}) {
  return (
    <main className="flex min-h-screen flex-1 flex-col items-center justify-center bg-slate-50 px-4 py-10 text-slate-900">
      <BrandMark withName className="mb-8" />
      <div className="mb-6 text-center">
        <h1 className="text-3xl font-extrabold tracking-tight">{title}</h1>
        {subtitle && <p className="mt-2 text-slate-500">{subtitle}</p>}
      </div>
      <div className="w-full max-w-[420px] rounded-3xl border border-slate-200/70 bg-white p-6 shadow-xl shadow-slate-900/5 sm:p-8">
        {children}
      </div>
    </main>
  );
}

export function FieldLabel({ htmlFor, children }: { htmlFor: string; children: React.ReactNode }) {
  return (
    <label htmlFor={htmlFor} className="text-xs font-semibold tracking-wider text-slate-500 uppercase">
      {children}
    </label>
  );
}

export const inputClass =
  "h-12 w-full rounded-2xl border border-slate-200 bg-white px-4 text-base outline-none transition focus:border-brand focus:ring-4 focus:ring-teal-100 disabled:bg-slate-50";

export const primaryButtonClass =
  "flex h-12 w-full items-center justify-center gap-2 rounded-2xl bg-brand text-base font-bold text-white shadow-lg shadow-teal-900/20 hover:bg-brand-dark disabled:bg-slate-300 disabled:shadow-none";
