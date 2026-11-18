import { createServerClient } from "@supabase/ssr";
import { NextResponse, type NextRequest } from "next/server";

import { AUTH_ENABLED, isPublicPath, SUPABASE_KEY, SUPABASE_URL } from "@/lib/supabase/config";

/**
 * Comprobacion optimista de sesion (ver docs de Next: "Optimistic checks with
 * Proxy"): sin sesion valida, las rutas privadas redirigen a /login.
 *
 * Tambien refresca el token de Supabase si vencio y reescribe sus cookies.
 * `getClaims()` verifica el JWT localmente con el JWKS del proyecto, sin ir a
 * la base en cada request. La autorizacion real (rol, sucursal) la hace el
 * backend en cada llamada.
 */
export async function proxy(request: NextRequest) {
  if (!AUTH_ENABLED) return NextResponse.next();

  let response = NextResponse.next({ request });
  const supabase = createServerClient(SUPABASE_URL, SUPABASE_KEY, {
    cookies: {
      getAll() {
        return request.cookies.getAll();
      },
      setAll(cookiesToSet) {
        for (const { name, value } of cookiesToSet) request.cookies.set(name, value);
        response = NextResponse.next({ request });
        for (const { name, value, options } of cookiesToSet) response.cookies.set(name, value, options);
      },
    },
  });

  // Si Supabase no responde (red caída, JWKS inalcanzable), se trata como sin
  // sesión: la ruta privada manda a /login en vez de devolver un 500.
  let signedIn = false;
  try {
    const { data } = await supabase.auth.getClaims();
    signedIn = Boolean(data?.claims?.sub);
  } catch (err) {
    console.error("proxy: no se pudo verificar la sesión", err);
  }
  const { pathname, search } = request.nextUrl;

  if (!signedIn && !isPublicPath(pathname)) {
    const login = new URL("/login", request.url);
    if (pathname !== "/") login.searchParams.set("next", `${pathname}${search}`);
    return NextResponse.redirect(login);
  }
  return response;
}

export const config = {
  // Todo salvo el proxy al backend, assets de Next y archivos estaticos.
  matcher: ["/((?!api|_next/static|_next/image|favicon.ico|.*\\.(?:png|jpg|jpeg|svg|webp|ico)$).*)"],
};
