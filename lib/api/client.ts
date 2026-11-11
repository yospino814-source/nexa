import type { ApiErrorBody } from "./types";

/**
 * El backend se consume por `/api` (proxy de Next, ver next.config.ts).
 * Sin `NEXT_PUBLIC_USE_BACKEND=true` la app usa datos mock.
 */
export const API_URL = "/api";
export const USE_MOCKS = process.env.NEXT_PUBLIC_USE_BACKEND !== "true";

export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
    public code?: string,
    public data?: unknown,
  ) {
    super(message);
  }
}

let accessToken: string | null = null;
let tokenProvider: (() => Promise<string | null>) | null = null;
let onUnauthorized: (() => void) | null = null;

/** Fija un JWT fijo (pruebas). En la app se usa `setTokenProvider`. */
export function setAccessToken(token: string | null) {
  accessToken = token;
}

/**
 * Funcion que entrega el JWT vigente de Supabase en cada request (refrescado
 * si vencio). La registra el store de sesion al arrancar.
 */
export function setTokenProvider(provider: (() => Promise<string | null>) | null) {
  tokenProvider = provider;
}

/** Se llama cuando el backend responde 401 (sesion vencida o revocada). */
export function setUnauthorizedHandler(handler: (() => void) | null) {
  onUnauthorized = handler;
}

export async function request<T>(path: string, init: RequestInit = {}): Promise<T> {
  const token = accessToken ?? (tokenProvider ? await tokenProvider() : null);
  const isFormData = typeof FormData !== "undefined" && init.body instanceof FormData;
  const response = await fetch(`${API_URL}${path}`, {
    ...init,
    headers: {
      ...(isFormData ? {} : { "content-type": "application/json" }),
      ...(token ? { authorization: `Bearer ${token}` } : {}),
      ...init.headers,
    },
  });

  if (response.status === 401) onUnauthorized?.();
  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as ApiErrorBody | null;
    throw new ApiError(response.status, body?.error ?? response.statusText, body?.code, body?.data);
  }
  // 204 (reposicion, ajuste, punto de reorden) no trae cuerpo.
  if (response.status === 204) return undefined as T;
  return (await response.json()) as T;
}

export function saveBlob(blob: Blob, filename: string) {
  const url = URL.createObjectURL(blob);
  const a = document.createElement("a");
  a.href = url;
  a.download = filename;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
}

function filenameFromDisposition(header: string | null): string | null {
  if (!header) return null;
  const utf8Match = /filename\*=UTF-8''([^;]+)/i.exec(header);
  if (utf8Match?.[1]) {
    try {
      return decodeURIComponent(utf8Match[1].replace(/"/g, "").trim());
    } catch {
      return utf8Match[1].replace(/"/g, "").trim();
    }
  }
  const plainMatch = /filename="?([^";]+)"?/i.exec(header);
  return plainMatch?.[1]?.trim() ?? null;
}

export async function downloadBlob(path: string, fallbackFilename: string): Promise<void> {
  const token = accessToken ?? (tokenProvider ? await tokenProvider() : null);
  const response = await fetch(`${API_URL}${path}`, {
    headers: {
      ...(token ? { authorization: `Bearer ${token}` } : {}),
    },
  });

  if (response.status === 401) onUnauthorized?.();
  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as ApiErrorBody | null;
    throw new ApiError(response.status, body?.error ?? response.statusText, body?.code, body?.data);
  }

  const blob = await response.blob();
  const filename = filenameFromDisposition(response.headers.get("content-disposition")) ?? fallbackFilename;
  saveBlob(blob, filename);
}
