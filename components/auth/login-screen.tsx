"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { Eye, EyeOff, Loader2, MailCheck } from "lucide-react";

import { getSupabase } from "@/lib/supabase/client";
import { AUTH_ENABLED } from "@/lib/supabase/config";
import { homeFor, rememberDevice, useSession } from "@/lib/stores/session";

import { AuthShell, FieldLabel, inputClass, primaryButtonClass } from "./auth-shell";
import { BranchPicker } from "./branch-picker";

/** Solo rutas internas: evita redirecciones abiertas con ?next=https://... */
function safeNext(next: string | null): string | null {
  return next && next.startsWith("/") && !next.startsWith("//") ? next : null;
}

function loginErrorMessage(message: string): string {
  if (/email not confirmed/i.test(message)) return "Confirmá tu email antes de entrar (revisá tu correo).";
  if (/too many|rate limit/i.test(message)) return "Demasiados intentos. Esperá un momento y probá de nuevo.";
  if (/fetch|network/i.test(message)) return "No se pudo conectar. Revisá tu conexión.";
  return "Email o contraseña incorrectos";
}

/** Pantalla 11: iniciar sesion (+ elegir sucursal si trabaja en varias). */
export function LoginScreen() {
  const router = useRouter();
  const params = useSearchParams();
  const next = safeNext(params.get("next"));
  const { status, branches, branchId, boot } = useSession();

  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [remember, setRemember] = useState(true);
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [resetSent, setResetSent] = useState(false);
  const [signedIn, setSignedIn] = useState(false);

  function goHome(chosen: string | null = branchId) {
    const role = branches.find((b) => b.id === chosen)?.role ?? null;
    router.replace(next ?? homeFor(role));
  }

  // Tras el login: ir a la app cuando la sesion queda lista. Sin negocio
  // asignado, la app (SessionGate) explica que falta y ofrece salir.
  useEffect(() => {
    if (!signedIn) return;
    if (status === "ready") goHome();
    else if (status === "no-business") router.replace(next ?? "/panel");
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [signedIn, status]);

  async function submit(e: React.FormEvent) {
    e.preventDefault();
    const supabase = getSupabase();
    if (!supabase) return;
    setSubmitting(true);
    setError(null);
    const { error: authError } = await supabase.auth.signInWithPassword({ email: email.trim(), password });
    if (authError) {
      setError(loginErrorMessage(authError.message));
      setSubmitting(false);
      return;
    }
    rememberDevice(remember);
    await boot();
    if (useSession.getState().status === "error") {
      setError(useSession.getState().error ?? "No se pudo conectar con el servidor.");
    } else {
      setSignedIn(true);
    }
    setSubmitting(false);
  }

  async function forgotPassword() {
    const supabase = getSupabase();
    if (!supabase) return;
    if (!email.trim()) {
      setError("Escribí tu email y tocá de nuevo “¿Olvidaste tu contraseña?”.");
      return;
    }
    setError(null);
    const { error: resetError } = await supabase.auth.resetPasswordForEmail(email.trim(), {
      redirectTo: `${window.location.origin}/login/restablecer`,
    });
    if (resetError) setError(loginErrorMessage(resetError.message));
    else setResetSent(true);
  }

  if (!AUTH_ENABLED) {
    return (
      <AuthShell title="Modo demo" subtitle="El login se activa al configurar Supabase">
        <div className="space-y-4 text-sm text-slate-600">
          <p>
            Faltan <code className="rounded bg-slate-100 px-1">NEXT_PUBLIC_SUPABASE_URL</code> y{" "}
            <code className="rounded bg-slate-100 px-1">NEXT_PUBLIC_SUPABASE_PUBLISHABLE_KEY</code> en{" "}
            <code className="rounded bg-slate-100 px-1">.env.local</code>. Mientras tanto la app usa datos de prueba.
          </p>
          <Link href="/panel" className={primaryButtonClass}>
            Entrar al demo
          </Link>
        </div>
      </AuthShell>
    );
  }

  if (signedIn && status === "choose-branch") {
    return (
      <AuthShell title="¿Dónde vas a trabajar hoy?" subtitle="Elegí la sucursal para esta sesión">
        <BranchPicker onChosen={(id) => goHome(id)} />
      </AuthShell>
    );
  }

  if (resetSent) {
    return (
      <AuthShell title="Revisá tu correo">
        <div className="space-y-5 text-center">
          <MailCheck className="mx-auto size-12 rounded-2xl bg-teal-50 p-2.5 text-brand" />
          <p className="text-slate-600">
            Si <strong>{email}</strong> tiene una cuenta, te enviamos un enlace para crear una contraseña nueva.
          </p>
          <button type="button" onClick={() => setResetSent(false)} className={primaryButtonClass}>
            Volver a entrar
          </button>
        </div>
      </AuthShell>
    );
  }

  return (
    <AuthShell title="Bienvenido de nuevo" subtitle="Entrá para seguir vendiendo">
      <form onSubmit={submit} className="space-y-5" noValidate>
        {error && (
          <p role="alert" className="rounded-2xl border border-rose-200 bg-rose-50 px-4 py-3 text-sm font-medium text-rose-700">
            {error}
          </p>
        )}

        <div className="space-y-1.5">
          <FieldLabel htmlFor="email">Email</FieldLabel>
          <input
            id="email"
            type="email"
            autoComplete="email"
            autoFocus
            required
            placeholder="usuario@email.com"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            disabled={submitting}
            className={inputClass}
          />
        </div>

        <div className="space-y-1.5">
          <div className="flex items-center justify-between">
            <FieldLabel htmlFor="password">Contraseña</FieldLabel>
            <button type="button" onClick={forgotPassword} className="text-xs font-semibold text-brand hover:underline">
              ¿Olvidaste tu contraseña?
            </button>
          </div>
          <div className="relative">
            <input
              id="password"
              type={showPassword ? "text" : "password"}
              autoComplete="current-password"
              required
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              disabled={submitting}
              className={`${inputClass} pr-12`}
            />
            <button
              type="button"
              onClick={() => setShowPassword((v) => !v)}
              aria-label={showPassword ? "Ocultar contraseña" : "Mostrar contraseña"}
              className="absolute inset-y-0 right-0 flex w-12 items-center justify-center text-slate-400 hover:text-slate-600"
            >
              {showPassword ? <EyeOff className="size-5" /> : <Eye className="size-5" />}
            </button>
          </div>
        </div>

        <label className="flex items-center gap-2 text-sm text-slate-600">
          <input
            type="checkbox"
            checked={remember}
            onChange={(e) => setRemember(e.target.checked)}
            className="size-4 rounded accent-[var(--brand)]"
          />
          Recordarme en este equipo
        </label>

        <button type="submit" disabled={submitting || !email || !password} className={primaryButtonClass}>
          {submitting ? (
            <>
              <Loader2 className="size-5 animate-spin" /> Entrando…
            </>
          ) : (
            "Entrar"
          )}
        </button>

        <p className="text-center text-sm text-slate-500">
          ¿No tenés cuenta?{" "}
          <Link href="/registro" className="font-semibold text-brand hover:underline">
            Crear cuenta
          </Link>
        </p>
      </form>
    </AuthShell>
  );
}
