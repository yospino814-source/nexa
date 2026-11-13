"use client";

import { useEffect, useState } from "react";

import { api, type BusinessInfo } from "@/lib/api";

/** Una sola carga por sesión de la pestaña: los datos del negocio casi no cambian. */
let cached: Promise<BusinessInfo | null> | null = null;

function loadBusiness(): Promise<BusinessInfo | null> {
  if (!cached) {
    cached = api.business.get().catch(() => {
      cached = null;
      return null;
    });
  }
  return cached;
}

/** Datos del negocio para la tirilla (F1.15); `null` mientras cargan o si fallan. */
export function useBusinessInfo(): BusinessInfo | null {
  const [info, setInfo] = useState<BusinessInfo | null>(null);
  useEffect(() => {
    let alive = true;
    loadBusiness().then((b) => alive && setInfo(b));
    return () => {
      alive = false;
    };
  }, []);
  return info;
}
