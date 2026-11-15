import type { NextConfig } from "next";

/**
 * URL del backend Rust, solo del lado del servidor. El navegador siempre llama
 * a `/api/*` (mismo origen) y Next reenvia al backend: no hace falta CORS.
 */
const BACKEND_URL = process.env.BACKEND_URL?.replace(/\/$/, "");

const isDev = process.env.NODE_ENV !== "production";
const SUPABASE_ORIGIN = (() => {
  try {
    return new URL(process.env.NEXT_PUBLIC_SUPABASE_URL ?? "").origin;
  } catch {
    return "";
  }
})();
const TURNSTILE = "https://challenges.cloudflare.com";

/**
 * Politica de contenido. Es estatica (sin nonce) porque la app usa
 * prerenderizado parcial y cache de componentes, y un nonce obliga a renderizar
 * cada pagina en cada request. Lo importante para un POS queda cubierto: no se
 * puede meter en un iframe de otro sitio, solo carga scripts propios y de
 * Turnstile, y solo habla con el mismo origen y con Supabase Auth.
 */
const CSP = [
  "default-src 'self'",
  `script-src 'self' 'unsafe-inline' ${TURNSTILE}${isDev ? " 'unsafe-eval'" : ""}`,
  "style-src 'self' 'unsafe-inline'",
  "img-src 'self' data: blob:",
  "font-src 'self'",
  `connect-src 'self' ${SUPABASE_ORIGIN} ${TURNSTILE}${isDev ? " ws:" : ""}`.trim(),
  `frame-src ${TURNSTILE}`,
  "object-src 'none'",
  "base-uri 'self'",
  "form-action 'self'",
  "frame-ancestors 'none'",
  ...(isDev ? [] : ["upgrade-insecure-requests"]),
].join("; ");

const SECURITY_HEADERS = [
  { key: "Content-Security-Policy", value: CSP },
  { key: "X-Frame-Options", value: "DENY" },
  { key: "X-Content-Type-Options", value: "nosniff" },
  { key: "Referrer-Policy", value: "strict-origin-when-cross-origin" },
  // La camara queda para el lector de codigos del POS; lo demas, apagado.
  { key: "Permissions-Policy", value: "camera=(self), microphone=(), geolocation=(), payment=(), usb=()" },
  { key: "Cross-Origin-Opener-Policy", value: "same-origin" },
  ...(isDev ? [] : [{ key: "Strict-Transport-Security", value: "max-age=63072000; includeSubDomains" }]),
];

const nextConfig: NextConfig = {
  cacheComponents: true,
  poweredByHeader: false,
  async headers() {
    // /design (solo en desarrollo) sirve HTML de Stitch con sus propios scripts.
    return [{ source: "/((?!design/).*)", headers: SECURITY_HEADERS }];
  },
  async rewrites() {
    if (!BACKEND_URL) return [];
    return [{ source: "/api/:path*", destination: `${BACKEND_URL}/:path*` }];
  },
  partialPrefetching: true,
  turbopack: {
    rules: {
      "*.css": {
        loaders: ["@tailwindcss/turbopack"],
        as: "*.css",
      },
    },
  },
};

export default nextConfig;
