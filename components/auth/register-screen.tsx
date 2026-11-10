"use client";

import { useState } from "react";
import Link from "next/link";
import { useRouter, useSearchParams } from "next/navigation";
import { Eye, EyeOff, Loader2, MailCheck } from "lucide-react";

import { getSupabase } from "@/lib/supabase/client";
import { AUTH_ENABLED } from "@/lib/supabase/config";

import {
  AuthShell,
  FieldLabel,
  inputClass,
  primaryButtonClass,
} from "./auth-shell";

export function RegisterScreen() {
  const router = useRouter();
  const searchParams = useSearchParams();

  const [fullName, setFullName] = useState("");
  const [email, setEmail] = useState(() => searchParams.get("correo") ?? "");
  const [password, setPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");

  const [showPassword, setShowPassword] = useState(false);
  const [showConfirmPassword, setShowConfirmPassword] = useState(false);

  const [nameError, setNameError] = useState<string | null>(null);
  const [emailError, setEmailError] = useState<string | null>(null);
  const [passwordError, setPasswordError] = useState<string | null>(null);
  const [confirmPasswordError, setConfirmPasswordError] = useState<string | null>(null);

  const [generalError, setGeneralError] = useState<React.ReactNode | null>(null);
  const [submitting, setSubmitting] = useState(false);
  const [registeredEmail, setRegisteredEmail] = useState<string | null>(null);

  async function handleSubmit(e: React.FormEvent) {
    e.preventDefault();

    setNameError(null);
    setEmailError(null);
    setPasswordError(null);
    setConfirmPasswordError(null);
    setGeneralError(null);

    // Modo demo (AUTH_ENABLED falso): lleva directo a /onboarding
    if (!AUTH_ENABLED) {
      router.push("/onboarding");
      return;
    }

    // Validación local, en este orden exacto:
    const trimmedName = fullName.trim();
    const nameLen = Array.from(trimmedName).length;
    if (nameLen < 2 || nameLen > 80) {
      setNameError("Escribí tu nombre");
      return;
    }

    const trimmedEmail = email.trim();
    if (!trimmedEmail.includes("@")) {
      setEmailError("Revisá el correo");
      return;
    }

    if (password.length < 8) {
      setPasswordError("La contraseña debe tener al menos 8 caracteres");
      return;
    }

    if (password !== confirmPassword) {
      setConfirmPasswordError("Las contraseñas no coinciden");
      return;
    }

    const supabase = getSupabase();
    if (!supabase) {
      router.push("/onboarding");
      return;
    }

    setSubmitting(true);
    try {
      const { error } = await supabase.auth.signUp({
        email: trimmedEmail,
        password,
        options: {
          emailRedirectTo: `${window.location.origin}/onboarding`,
          data: { full_name: trimmedName },
        },
      });

      if (error) {
        const msg = error.message || "";
        const code = (error as unknown as { code?: string }).code || "";
        if (code === "user_already_exists" || /user already registered/i.test(msg)) {
          setGeneralError(
            <span>
              Ese correo ya tiene cuenta.{" "}
              <Link href="/login" className="font-semibold text-brand underline hover:text-brand-dark">
                Iniciá sesión
              </Link>
            </span>,
          );
        } else {
          setGeneralError("No pudimos crear la cuenta. Intentá de nuevo en unos minutos.");
        }
        return;
      }

      setRegisteredEmail(trimmedEmail);
    } catch {
      setGeneralError("No pudimos crear la cuenta. Intentá de nuevo en unos minutos.");
    } finally {
      setSubmitting(false);
    }
  }

  if (registeredEmail) {
    return (
      <AuthShell
        title="Creá tu cuenta"
        subtitle="Después configurás tu negocio en un minuto"
      >
        <div className="space-y-4 text-center">
          <MailCheck className="mx-auto size-12 rounded-2xl bg-teal-50 p-2 text-brand" />
          <p className="text-slate-600">
            Te enviamos un correo a <strong>{registeredEmail}</strong>. Abrí el enlace para seguir. Si no llega, revisá la carpeta de spam.
          </p>
          <div className="pt-2">
            <Link
              href="/login"
              className="text-sm font-semibold text-brand hover:text-brand-dark hover:underline"
            >
              Ir a iniciar sesión
            </Link>
          </div>
        </div>
      </AuthShell>
    );
  }

  return (
    <AuthShell
      title="Creá tu cuenta"
      subtitle="Después configurás tu negocio en un minuto"
    >
      <form onSubmit={handleSubmit} className="space-y-4" noValidate>
        {generalError && (
          <div className="rounded-2xl border border-red-200 bg-red-50 p-3 text-sm text-red-700">
            {generalError}
          </div>
        )}

        <div className="space-y-1.5">
          <FieldLabel htmlFor="reg-name">Tu nombre</FieldLabel>
          <input
            id="reg-name"
            type="text"
            required
            autoComplete="name"
            value={fullName}
            onChange={(e) => setFullName(e.target.value)}
            className={inputClass}
            placeholder="María López"
          />
          {nameError && (
            <p className="text-xs font-medium text-red-600">{nameError}</p>
          )}
        </div>

        <div className="space-y-1.5">
          <FieldLabel htmlFor="reg-email">Correo</FieldLabel>
          <input
            id="reg-email"
            type="email"
            required
            autoComplete="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            className={inputClass}
            placeholder="maria@negocio.com"
          />
          {emailError && (
            <p className="text-xs font-medium text-red-600">{emailError}</p>
          )}
        </div>

        <div className="space-y-1.5">
          <FieldLabel htmlFor="reg-password">Contraseña</FieldLabel>
          <div className="relative">
            <input
              id="reg-password"
              type={showPassword ? "text" : "password"}
              required
              autoComplete="new-password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              className={`${inputClass} pr-12`}
              placeholder="••••••••"
            />
            <button
              type="button"
              onClick={() => setShowPassword((v) => !v)}
              className="absolute top-1/2 right-3 -translate-y-1/2 p-1.5 text-slate-400 hover:text-slate-600"
              aria-label={showPassword ? "Ocultar contraseña" : "Ver contraseña"}
            >
              {showPassword ? <EyeOff className="size-4" /> : <Eye className="size-4" />}
            </button>
          </div>
          {passwordError && (
            <p className="text-xs font-medium text-red-600">{passwordError}</p>
          )}
        </div>

        <div className="space-y-1.5">
          <FieldLabel htmlFor="reg-confirm-password">Repetí la contraseña</FieldLabel>
          <div className="relative">
            <input
              id="reg-confirm-password"
              type={showConfirmPassword ? "text" : "password"}
              required
              autoComplete="new-password"
              value={confirmPassword}
              onChange={(e) => setConfirmPassword(e.target.value)}
              className={`${inputClass} pr-12`}
              placeholder="••••••••"
            />
            <button
              type="button"
              onClick={() => setShowConfirmPassword((v) => !v)}
              className="absolute top-1/2 right-3 -translate-y-1/2 p-1.5 text-slate-400 hover:text-slate-600"
              aria-label={showConfirmPassword ? "Ocultar contraseña" : "Ver contraseña"}
            >
              {showConfirmPassword ? <EyeOff className="size-4" /> : <Eye className="size-4" />}
            </button>
          </div>
          {confirmPasswordError && (
            <p className="text-xs font-medium text-red-600">{confirmPasswordError}</p>
          )}
        </div>

        <button
          type="submit"
          disabled={submitting}
          className={`${primaryButtonClass} mt-2`}
        >
          {submitting ? (
            <>
              <Loader2 className="size-5 animate-spin" /> Creando cuenta...
            </>
          ) : (
            "Crear cuenta"
          )}
        </button>

        <p className="pt-2 text-center text-sm text-slate-500">
          ¿Ya tenés cuenta?{" "}
          <Link
            href="/login"
            className="font-semibold text-brand hover:text-brand-dark hover:underline"
          >
            Iniciá sesión
          </Link>
        </p>
      </form>
    </AuthShell>
  );
}

