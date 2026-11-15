// Configuracion publica de Supabase Auth (va al navegador: solo URL y clave
// publica/anon, nunca la service_role).

export const SUPABASE_URL = process.env.NEXT_PUBLIC_SUPABASE_URL ?? "";
export const SUPABASE_KEY =
  process.env.NEXT_PUBLIC_SUPABASE_PUBLISHABLE_KEY ?? process.env.NEXT_PUBLIC_SUPABASE_ANON_KEY ?? "";

/** Sin URL y clave, la app corre sin login (modo demo con datos mock). */
export const AUTH_ENABLED = SUPABASE_URL !== "" && SUPABASE_KEY !== "";

/** Clave publica de Cloudflare Turnstile. Sin ella no se pide verificacion anti-bots. */
export const TURNSTILE_SITE_KEY = process.env.NEXT_PUBLIC_TURNSTILE_SITE_KEY ?? "";
export const CAPTCHA_ENABLED = TURNSTILE_SITE_KEY !== "";

/** Rutas que no exigen sesion. El resto queda protegido por proxy.ts.
 *  `/design` (referencias de Stitch) solo existe fuera de produccion. */
export const PUBLIC_PREFIXES = [
  "/login",
  "/registro",
  "/onboarding",
  "/tienda",
  ...(process.env.NODE_ENV === "production" ? [] : ["/design"]),
];

export function isPublicPath(pathname: string): boolean {
  return PUBLIC_PREFIXES.some((p) => pathname === p || pathname.startsWith(`${p}/`));
}
