"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import {
  AlertTriangle,
  Building,
  CheckCircle2,
  ChevronRight,
  CreditCard,
  FileText,
  Loader2,
  Package,
  Users,
} from "lucide-react";

import { api, ApiError, friendlyError } from "@/lib/api";
import { useSession } from "@/lib/stores/session";
import { AUTH_ENABLED } from "@/lib/supabase/config";

import {
  AuthShell,
  FieldLabel,
  inputClass,
  primaryButtonClass,
} from "./auth-shell";

export function OnboardingScreen() {
  const router = useRouter();
  const { status, error, name: sessionName, boot, invitations } = useSession();

  const [businessName, setBusinessName] = useState("");
  const [branchName, setBranchName] = useState("Principal");
  const [customName, setCustomName] = useState<string | null>(null);
  const fullName = customName ?? sessionName ?? "";

  const [businessNameError, setBusinessNameError] = useState<string | null>(null);
  const [branchNameError, setBranchNameError] = useState<string | null>(null);
  const [fullNameError, setFullNameError] = useState<string | null>(null);
  const [generalError, setGeneralError] = useState<string | null>(null);

  const [submitting, setSubmitting] = useState(false);
  const [success, setSuccess] = useState(false);

  useEffect(() => {
    boot();
  }, [boot]);

  useEffect(() => {
    if (!AUTH_ENABLED || success || submitting) return;
    if (status === "signed-out") {
      router.replace("/login?next=/onboarding");
    } else if (status === "ready" || status === "choose-branch") {
      router.replace("/panel");
    }
  }, [status, router, success, submitting]);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();

    setBusinessNameError(null);
    setBranchNameError(null);
    setFullNameError(null);
    setGeneralError(null);
    setSubmitting(true);

    try {
      await api.onboarding.create({
        business_name: businessName,
        branch_name: branchName,
        full_name: fullName,
      });

      await boot();
      setSuccess(true);
    } catch (err: unknown) {
      if (err instanceof ApiError) {
        if (err.code === "ONBOARDING_ALREADY_MEMBER") {
          router.replace("/panel");
          return;
        }

        if (err.code === "ONBOARDING_INVALID") {
          const field = (err.data as { field?: string } | undefined)?.field;
          const msg = friendlyError(err);
          if (field === "business_name") {
            setBusinessNameError(msg);
          } else if (field === "branch_name") {
            setBranchNameError(msg);
          } else if (field === "full_name") {
            setFullNameError(msg);
          } else {
            setGeneralError(msg);
          }
          return;
        }
      }

      setGeneralError("No pudimos crear el negocio. Intentá de nuevo en unos minutos.");
    } finally {
      setSubmitting(false);
    }
  }

  if (status === "loading") {
    return (
      <div className="flex min-h-screen flex-1 items-center justify-center bg-slate-50" aria-busy="true">
        <Loader2 className="size-8 animate-spin text-brand" aria-label="Cargando sesión" />
      </div>
    );
  }

  if (status === "error") {
    return (
      <AuthShell title="No pudimos iniciar">
        <div className="space-y-5 text-center">
          <AlertTriangle className="mx-auto size-12 rounded-2xl bg-amber-50 p-2.5 text-amber-600" />
          <p className="text-slate-600">{error}</p>
          <button
            onClick={() => boot()}
            className="flex h-12 w-full items-center justify-center rounded-2xl bg-brand font-semibold text-white hover:bg-brand-dark"
          >
            Reintentar
          </button>
        </div>
      </AuthShell>
    );
  }

  if (success) {
    const steps = [
      {
        title: "Completá los datos del negocio",
        href: "/configuracion/negocio",
        icon: FileText,
      },
      {
        title: "Cargá tus productos",
        href: "/inventario/importar",
        icon: Package,
      },
      {
        title: "Invitá a tus cajeros",
        href: "/configuracion/usuarios",
        icon: Users,
      },
      {
        title: "Abrí la caja y vendé",
        href: "/pos",
        icon: CreditCard,
      },
    ];

    return (
      <AuthShell
        title="Tu negocio está listo"
        subtitle="Ya podés empezar a usar Nexa POS"
      >
        <div className="space-y-6">
          <div className="flex items-center justify-center gap-2 rounded-2xl bg-teal-50 p-3 text-sm font-semibold text-teal-800">
            <CheckCircle2 className="size-5 text-teal-600" />
            Configuración inicial completa
          </div>

          <div className="space-y-2.5">
            <p className="text-xs font-semibold tracking-wider text-slate-500 uppercase">
              Primeros pasos recomendados
            </p>
            <div className="divide-y divide-slate-100 rounded-2xl border border-slate-200">
              {steps.map((s, idx) => {
                const Icon = s.icon;
                return (
                  <Link
                    key={s.href}
                    href={s.href}
                    className="flex items-center justify-between p-3.5 transition hover:bg-slate-50"
                  >
                    <div className="flex items-center gap-3">
                      <div className="flex size-7 items-center justify-center rounded-full bg-slate-100 text-xs font-bold text-slate-600">
                        {idx + 1}
                      </div>
                      <Icon className="size-4 text-slate-400" />
                      <span className="text-sm font-medium text-slate-800">
                        {s.title}
                      </span>
                    </div>
                    <ChevronRight className="size-4 text-slate-400" />
                  </Link>
                );
              })}
            </div>
          </div>

          <button
            onClick={() => router.push("/panel")}
            className={primaryButtonClass}
          >
            Ir al panel
          </button>
        </div>
      </AuthShell>
    );
  }

  return (
    <AuthShell
      title="Configurá tu negocio"
      subtitle="El nombre aparecerá en tus tirillas y documentos"
    >
      <form onSubmit={handleSubmit} className="space-y-4" noValidate>
        {invitations.length > 0 && (
          <div className="flex items-start gap-3 rounded-2xl border border-amber-200 bg-amber-50 p-4 text-xs text-amber-900">
            <AlertTriangle className="size-5 shrink-0 text-amber-600" />
            <div className="flex-1 space-y-2">
              <p>
                Tenés {invitations.length} {invitations.length === 1 ? "invitación pendiente" : "invitaciones pendientes"}. Si vas a trabajar en un negocio que ya existe, aceptala en vez de crear uno nuevo.
              </p>
              <button
                type="button"
                onClick={() => router.push("/panel")}
                className="rounded-full bg-amber-600 px-3 py-1 font-semibold text-white hover:bg-amber-700"
              >
                Ver invitaciones
              </button>
            </div>
          </div>
        )}

        {generalError && (
          <div className="rounded-2xl border border-red-200 bg-red-50 p-3 text-sm text-red-700">
            {generalError}
          </div>
        )}

        <div className="space-y-1.5">
          <FieldLabel htmlFor="onboarding-business">Nombre del negocio</FieldLabel>
          <input
            id="onboarding-business"
            type="text"
            required
            value={businessName}
            onChange={(e) => setBusinessName(e.target.value)}
            className={inputClass}
            placeholder="Licorera La 15"
          />
          <p className="text-xs text-slate-500">
            Así aparece en la tirilla. Ej.: Licorera La 15
          </p>
          {businessNameError && (
            <p className="text-xs font-medium text-red-600">{businessNameError}</p>
          )}
        </div>

        <div className="space-y-1.5">
          <FieldLabel htmlFor="onboarding-branch">Nombre de la sucursal</FieldLabel>
          <input
            id="onboarding-branch"
            type="text"
            required
            value={branchName}
            onChange={(e) => setBranchName(e.target.value)}
            className={inputClass}
            placeholder="Principal"
          />
          {branchNameError && (
            <p className="text-xs font-medium text-red-600">{branchNameError}</p>
          )}
        </div>

        <div className="space-y-1.5">
          <FieldLabel htmlFor="onboarding-fullname">Tu nombre</FieldLabel>
          <input
            id="onboarding-fullname"
            type="text"
            required
            value={fullName}
            onChange={(e) => setCustomName(e.target.value)}
            className={inputClass}
            placeholder="Pedro Pérez"
          />
          {fullNameError && (
            <p className="text-xs font-medium text-red-600">{fullNameError}</p>
          )}
        </div>

        <button
          type="submit"
          disabled={submitting}
          className={`${primaryButtonClass} mt-2`}
        >
          {submitting ? (
            <>
              <Loader2 className="size-5 animate-spin" /> Creando negocio...
            </>
          ) : (
            <>
              <Building className="size-4" /> Crear negocio
            </>
          )}
        </button>
      </form>
    </AuthShell>
  );
}

