import type { NextConfig } from "next";

/**
 * URL del backend Rust, solo del lado del servidor. El navegador siempre llama
 * a `/api/*` (mismo origen) y Next reenvia al backend: no hace falta CORS.
 */
const BACKEND_URL = process.env.BACKEND_URL?.replace(/\/$/, "");

const nextConfig: NextConfig = {
  cacheComponents: true,
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
