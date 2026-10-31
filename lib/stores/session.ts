"use client";

import { create } from "zustand";

import {
  api,
  ApiError,
  setTokenProvider,
  setUnauthorizedHandler,
  USE_MOCKS,
  type BranchAccess,
  type Role,
} from "@/lib/api";
import { AUTH_ENABLED } from "@/lib/supabase/config";
import { currentAccessToken, getSupabase } from "@/lib/supabase/client";

export type SessionStatus =
  | "loading"
  /** Sin sesion: ir a /login. */
  | "signed-out"
  /** Con sesion pero sin negocio asignado (falta onboarding o invitacion). */
  | "no-business"
  /** Trabaja en varias sucursales y aun no eligio. */
  | "choose-branch"
  | "ready"
  | "error";

type SessionState = {
  status: SessionStatus;
  error: string | null;
  email: string | null;
  name: string | null;
  branches: BranchAccess[];
  branchId: string | null;
  boot: () => Promise<void>;
  selectBranch: (branchId: string) => void;
  signOut: () => Promise<void>;
};

const BRANCH_KEY = "nexa.branch";
const REMEMBER_KEY = "nexa.remember";
const ALIVE_KEY = "nexa.alive";

export const ROLE_LABELS: Record<Role, string> = {
  DUENO: "Dueño",
  ADMIN_CAJA: "Administrador",
  CAJERO: "Cajero",
  MESERO: "Mesero",
  COCINA: "Cocina",
};

/** Pantalla de inicio segun el rol. */
export function homeFor(role: Role | null): string {
  if (role === "CAJERO") return "/pos";
  if (role === "MESERO") return "/mesas";
  if (role === "COCINA") return "/cocina";
  return "/panel";
}

/**
 * "Recordarme en este equipo": si se desmarco, la sesion dura solo mientras
 * el navegador siga abierto (sessionStorage se borra al cerrarlo).
 */
export function rememberDevice(remember: boolean) {
  localStorage.setItem(REMEMBER_KEY, remember ? "1" : "0");
  sessionStorage.setItem(ALIVE_KEY, "1");
}

function sessionExpiredByBrowserClose(): boolean {
  return localStorage.getItem(REMEMBER_KEY) === "0" && sessionStorage.getItem(ALIVE_KEY) !== "1";
}

let booting: Promise<void> | null = null;

export const useSession = create<SessionState>((set, get) => ({
  status: "loading",
  error: null,
  email: null,
  name: null,
  branches: [],
  branchId: null,

  boot() {
    // Varias pantallas pueden pedir el arranque a la vez: se hace una sola vez.
    booting ??= bootOnce(set).finally(() => {
      booting = null;
    });
    return booting;
  },

  selectBranch(branchId) {
    if (!get().branches.some((b) => b.id === branchId)) return;
    localStorage.setItem(BRANCH_KEY, branchId);
    set({ branchId, status: "ready" });
  },

  async signOut() {
    await getSupabase()?.auth.signOut();
    localStorage.removeItem(BRANCH_KEY);
    set({ status: "signed-out", email: null, name: null, branches: [], branchId: null });
  },
}));

type Setter = (partial: Partial<SessionState>) => void;

async function bootOnce(set: Setter) {
  set({ status: "loading", error: null });

  if (!AUTH_ENABLED && !USE_MOCKS) {
    set({
      status: "error",
      error: "Falta configurar Supabase (NEXT_PUBLIC_SUPABASE_URL y la clave pública) para usar el backend.",
    });
    return;
  }

  let email: string | null = null;
  let name: string | null = null;
  const supabase = getSupabase();
  if (supabase) {
    setTokenProvider(currentAccessToken);
    setUnauthorizedHandler(() => set({ status: "signed-out" }));

    if (sessionExpiredByBrowserClose()) await supabase.auth.signOut();
    const { data } = await supabase.auth.getSession();
    const user = data.session?.user;
    if (!user) {
      set({ status: "signed-out" });
      return;
    }
    email = user.email ?? null;
    name = (user.user_metadata?.full_name as string | undefined) ?? email?.split("@")[0] ?? null;
  } else {
    // Modo demo sin Supabase.
    name = "Usuario demo";
  }

  try {
    const branches = await api.session.branches();
    if (branches.length === 0) {
      set({ status: "no-business", email, name, branches });
      return;
    }
    const stored = localStorage.getItem(BRANCH_KEY);
    const branchId =
      branches.find((b) => b.id === stored)?.id ?? (branches.length === 1 ? branches[0].id : null);
    set({ status: branchId ? "ready" : "choose-branch", email, name, branches, branchId });
  } catch (e) {
    if (e instanceof ApiError && e.status === 403) {
      set({ status: "no-business", email, name, branches: [] });
    } else if (e instanceof ApiError && e.status === 401) {
      set({ status: "signed-out" });
    } else {
      set({ status: "error", error: "No se pudo conectar con el servidor.", email, name });
    }
  }
}

/** Sucursal elegida (con su rol). Solo usar dentro de `<SessionGate>`. */
export function useCurrentBranch(): BranchAccess {
  const { branches, branchId } = useSession();
  const branch = branches.find((b) => b.id === branchId);
  if (!branch) throw new Error("useCurrentBranch fuera de una sesion lista");
  return branch;
}

export function initials(name: string | null): string {
  if (!name) return "?";
  const parts = name.trim().split(/\s+/);
  return (parts[0][0] + (parts[1]?.[0] ?? "")).toUpperCase();
}
