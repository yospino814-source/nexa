export type PersonType = "NATURAL" | "JURIDICA";

/**
 * Regla B5: Tipo de persona por defecto según tipo y número de documento.
 * - Solo para NIT con 9 dígitos que empiece por 8 o 9 es JURIDICA.
 * - Con cualquier otro documento o formato es NATURAL.
 */
export function defaultPersonType(
  docType?: string | null,
  docNumber?: string | null
): PersonType {
  if (docType?.toUpperCase().trim() !== "NIT" || !docNumber) {
    return "NATURAL";
  }
  const digits = docNumber.replace(/\D/g, "");
  if (digits.length === 9 && (digits.startsWith("8") || digits.startsWith("9"))) {
    return "JURIDICA";
  }
  return "NATURAL";
}

