import { create } from "zustand";

/**
 * Version del estado de caja. Quien abre, cierra o mueve efectivo llama a
 * `bumpCash()`; las vistas que muestran la caja (barra superior, /caja) usan
 * `useCashVersion()` como parte de la clave de `useApi` para recargar.
 */
const useCashStore = create<{ version: number; bump: () => void }>((set) => ({
  version: 0,
  bump: () => set((s) => ({ version: s.version + 1 })),
}));

export const useCashVersion = () => useCashStore((s) => s.version);
export const bumpCash = () => useCashStore.getState().bump();
