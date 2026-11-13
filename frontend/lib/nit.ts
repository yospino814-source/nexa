const WEIGHTS = [3, 7, 13, 17, 19, 23, 29, 37, 41, 43, 47, 53, 59, 67, 71];

/**
 * Calcula el dígito de verificación (DV) para un NIT en Colombia usando el algoritmo DIAN (módulo 11).
 * Algoritmo equivalente al implementado en el backend (`procurement::nit_dv`).
 * Retorna null si el NIT está vacío o no contiene únicamente dígitos.
 */
export function nitDv(nit: string): number | null {
  const trimmed = nit.trim();
  if (!trimmed || !/^\d+$/.test(trimmed)) {
    return null;
  }
  const digits = trimmed.split('').map((c) => parseInt(c, 10));
  let sum = 0;
  for (let i = 0; i < digits.length; i++) {
    const digit = digits[digits.length - 1 - i];
    if (i < WEIGHTS.length) {
      sum += digit * WEIGHTS[i];
    }
  }
  const r = sum % 11;
  return r === 0 || r === 1 ? r : 11 - r;
}

