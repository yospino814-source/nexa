"use client";

import { createBrowserClient } from "@supabase/ssr";
import type { SupabaseClient } from "@supabase/supabase-js";

import { AUTH_ENABLED, SUPABASE_KEY, SUPABASE_URL } from "./config";

let client: SupabaseClient | null = null;

/**
 * Cliente de Supabase del navegador (singleton). Guarda la sesion en cookies,
 * que proxy.ts lee para proteger las rutas. `null` si Auth no esta configurado.
 */
export function getSupabase(): SupabaseClient | null {
  if (!AUTH_ENABLED) return null;
  client ??= createBrowserClient(SUPABASE_URL, SUPABASE_KEY);
  return client;
}

/** JWT vigente (se refresca solo si vencio) para mandarlo al backend. */
export async function currentAccessToken(): Promise<string | null> {
  const supabase = getSupabase();
  if (!supabase) return null;
  const { data } = await supabase.auth.getSession();
  return data.session?.access_token ?? null;
}
