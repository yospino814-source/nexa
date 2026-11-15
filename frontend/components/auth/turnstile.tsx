"use client";

import { useEffect, useRef } from "react";

import { TURNSTILE_SITE_KEY } from "@/lib/supabase/config";

/**
 * Verificación anti-bots de Cloudflare Turnstile. Casi siempre es invisible:
 * no pide resolver imágenes. Supabase Auth valida el token con la clave
 * secreta (Authentication → Bot and Abuse Protection).
 *
 * Cada token sirve una sola vez: después de usarlo, cambiá `resetKey` para
 * pedir uno nuevo.
 */

type TurnstileApi = {
  render: (el: HTMLElement, opts: Record<string, unknown>) => string;
  remove: (id: string) => void;
};

declare global {
  interface Window {
    turnstile?: TurnstileApi;
  }
}

const SCRIPT_SRC = "https://challenges.cloudflare.com/turnstile/v0/api.js?render=explicit";
let scriptPromise: Promise<void> | null = null;

function loadScript(): Promise<void> {
  if (window.turnstile) return Promise.resolve();
  scriptPromise ??= new Promise((resolve, reject) => {
    const s = document.createElement("script");
    s.src = SCRIPT_SRC;
    s.async = true;
    s.defer = true;
    s.onload = () => resolve();
    s.onerror = () => {
      scriptPromise = null;
      reject(new Error("turnstile"));
    };
    document.head.appendChild(s);
  });
  return scriptPromise;
}

export function Turnstile({
  onToken,
  resetKey = 0,
  action,
}: {
  onToken: (token: string | null) => void;
  resetKey?: number;
  action?: string;
}) {
  const box = useRef<HTMLDivElement>(null);
  const tokenCb = useRef(onToken);
  tokenCb.current = onToken;

  useEffect(() => {
    if (!TURNSTILE_SITE_KEY || !box.current) return;
    let id: string | null = null;
    let cancelled = false;
    tokenCb.current(null);
    loadScript()
      .then(() => {
        if (cancelled || !box.current || !window.turnstile) return;
        id = window.turnstile.render(box.current, {
          sitekey: TURNSTILE_SITE_KEY,
          action,
          language: "es",
          appearance: "interaction-only",
          callback: (t: string) => tokenCb.current(t),
          "expired-callback": () => tokenCb.current(null),
          "error-callback": () => tokenCb.current(null),
        });
      })
      .catch(() => tokenCb.current(null));
    return () => {
      cancelled = true;
      if (id && window.turnstile) window.turnstile.remove(id);
    };
  }, [resetKey, action]);

  if (!TURNSTILE_SITE_KEY) return null;
  return <div ref={box} className="flex justify-center empty:hidden" />;
}
