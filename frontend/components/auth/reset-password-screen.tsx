"use client";

import { useEffect, useState } from "react";
import Link from "next/link";
import { useRouter } from "next/navigation";
import { Loader2 } from "lucide-react";

import { getSupabase } from "@/lib/supabase/client";
import { AUTH_ENABLED } from "@/lib/supabase/config";
import { useSession } from "@/lib/stores/session";

import { AuthShell, FieldLabel, inputClass, primaryButtonClass } from "./auth-shell";

const MIN_LENGTH = 8;

/**
 * Destino del enlace de "¿Olvidaste tu contraseña?". El cliente de Supabase
 * canjea el `?code=` del enlace por una sesion de recuperacion al cargar.
 */
export function ResetPasswordScreen() {
  const router = useRouter();
  const boot = useSession((s) => s.boot);
  // null = verificando el enlace; sin Supabase configurado no hay nada que verificar.
  const [ready, setReady] = useState<boolean | null>(AUTH_ENABLED ? null : false);
  const [password, setPassword] = useState("");
  const [confirm, setConfirm] = useState("");
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    const supabase = getSupabase();
    if (!supabase) return;
    const { data: sub } = supabase.auth.onAuthStateChange((event, session) => {
      if (event === "PASSWORD_RECOVERY" || session) setReady(true);
    });
    // Si el canje ya ocurrio antes de suscribirse.
    supabase.auth.getSession().then(({ data }) => setReady((prev) => prev ?? Boolean(data.session)));
    const timeout = setTimeout(() => setReady((prev) => prev ?? false), 4000);
    return () => {
      sub.subscription.unsubscribe();
      clearTimeout(timeout);
    };
  }, []);

  async function save(e: React.FormEvent) {
    e.preventDefault();
    if (password.length < MIN_LENGTH) return setError(`La contraseña debe tener al menos ${MIN_LENGTH} caracteres.`);
    if (password !== confirm) return setError("Las contraseñas no coinciden.");
    const supabase = getSupabase();
    if (!supabase) return;
    setSaving(true);
    setError(null);
    const { error: updateError } = await supabase.auth.updateUser({ password });
    setSaving(false);
    if (updateError) {
      setError("No se pudo guardar la contraseña. Pedí un enlace nuevo.");
      return;
    }
    await boot();
    router.replace("/");
  }

  if (ready === null) {
    return (
      <AuthShell title="Verificando enlace…">
        <Loader2 className="mx-auto size-8 animate-spin text-brand" />
      </AuthShell>
    );
  }

  if (!ready) {
    return (
      <AuthShell title="Enlace vencido o inválido" subtitle="Pedí uno nuevo desde la pantalla de inicio de sesión">
        <Link href="/login" className={primaryButtonClass}>
          Volver a iniciar sesión
        </Link>
      </AuthShell>
    );
  }

  return (
    <AuthShell title="Creá una contraseña nueva">
      <form onSubmit={save} className="space-y-5" noValidate>
        {error && (
          <p role="alert" className="rounded-2xl border border-rose-200 bg-rose-50 px-4 py-3 text-sm font-medium text-rose-700">
            {error}
          </p>
        )}
        <div className="space-y-1.5">
          <FieldLabel htmlFor="new-password">Contraseña nueva</FieldLabel>
          <input
            id="new-password"
            type="password"
            autoComplete="new-password"
            autoFocus
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            className={inputClass}
          />
        </div>
        <div className="space-y-1.5">
          <FieldLabel htmlFor="confirm-password">Repetila</FieldLabel>
          <input
            id="confirm-password"
            type="password"
            autoComplete="new-password"
            value={confirm}
            onChange={(e) => setConfirm(e.target.value)}
            className={inputClass}
          />
        </div>
        <button type="submit" disabled={saving || !password} className={primaryButtonClass}>
          {saving && <Loader2 className="size-5 animate-spin" />} Guardar y entrar
        </button>
      </form>
    </AuthShell>
  );
}
