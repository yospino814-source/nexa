import { Onest, Unbounded } from "next/font/google";

import { AuthBackdrop } from "./auth-backdrop";

const onest = Onest({ subsets: ["latin"], variable: "--font-onest", display: "swap" });
const unbounded = Unbounded({ subsets: ["latin"], weight: ["500", "600", "700"], variable: "--font-unbounded", display: "swap" });

/**
 * Marco de las pantallas de acceso (ingreso, registro, configuración inicial):
 * fondo oscuro con la pared de productos, el logo y una tarjeta de vidrio.
 */
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
    <main
      className={`auth-dark relative flex min-h-screen flex-1 items-center justify-center overflow-hidden bg-[#0E0F14] px-4 py-10 text-white ${onest.variable} ${unbounded.variable}`}
    >
      <AuthBackdrop />
      <div className="relative flex w-full max-w-[420px] flex-col items-center gap-1">
        <div className="auth-in-logo -mb-[18px] size-[150px]">
          {/* eslint-disable-next-line @next/next/no-img-element */}
          <img
            src="/brand/nexa-logo.webp"
            alt="Nexa"
            width={150}
            height={150}
            fetchPriority="high"
            className="size-[150px] object-cover [mask-image:radial-gradient(circle,#000_52%,transparent_70%)]"
          />
        </div>
        <div className="auth-in-form w-full rounded-[28px] bg-[rgba(20,21,29,.72)] p-8 shadow-[0_0_0_1px_rgba(255,255,255,.12),inset_0_1px_0_rgba(255,255,255,.08),0_40px_90px_-24px_rgba(0,0,0,.75)] backdrop-blur-[22px] backdrop-saturate-[1.4]">
          <div className="mb-5 flex flex-col gap-1.5 text-center">
            <h1 className="auth-display text-2xl font-semibold tracking-[-0.03em]">{title}</h1>
            {subtitle && <p className="text-[15px] text-[#B9B6C8]">{subtitle}</p>}
          </div>
          {children}
        </div>
      </div>
    </main>
  );
}

export function FieldLabel({ htmlFor, children }: { htmlFor: string; children: React.ReactNode }) {
  return (
    <label htmlFor={htmlFor} className="text-sm font-medium text-[#E6E4EE]">
      {children}
    </label>
  );
}

export const inputClass =
  "h-[52px] w-full rounded-2xl border border-white/16 bg-white/6 px-4 text-base text-white outline-none transition placeholder:text-[#8E8BA0] focus:border-[rgba(201,184,255,.9)] focus:ring-4 focus:ring-[rgba(108,62,245,.35)] disabled:opacity-60";

export const primaryButtonClass =
  "flex h-[54px] w-full items-center justify-center gap-2.5 rounded-2xl bg-white text-base font-semibold text-[#15131C] shadow-[0_14px_34px_-10px_rgba(108,62,245,.7)] transition-transform hover:-translate-y-px active:scale-[0.98] disabled:translate-y-0 disabled:opacity-60 disabled:shadow-none";
