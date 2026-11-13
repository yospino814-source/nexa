"use client";

import { useCallback, useEffect, useRef, useState } from "react";

/**
 * Carga datos de `api` en un componente cliente, con estado de carga y error.
 * `key` identifica la consulta: si cambia (p. ej. otra sucursal), se recarga.
 */
export function useApi<T>(load: () => Promise<T>, key = "") {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<Error | null>(null);
  const [loading, setLoading] = useState(true);
  const loadRef = useRef(load);

  useEffect(() => {
    loadRef.current = load;
  });

  const reload = useCallback(async () => {
    setLoading(true);
    try {
      setData(await loadRef.current());
      setError(null);
    } catch (e) {
      setError(e instanceof Error ? e : new Error(String(e)));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    // eslint-disable-next-line react-hooks/set-state-in-effect
    reload();
  }, [reload, key]);

  return { data, error, loading, reload };
}
