import { API_URL, USE_MOCKS } from "@/lib/api/client";

/** Modo demo: las fotos subidas quedan en memoria como data URI. */
export const mockPhotos = new Map<string, string>();

/** URL de la foto del producto, o null si no tiene. */
export function productImageUrl(productId: string, version: number | null | undefined): string | null {
  if (USE_MOCKS) return mockPhotos.get(productId) ?? null;
  if (version == null) return null;
  return `${API_URL}/v1/public/product-images/${productId}?v=${version}`;
}

const MAX_SIDE = 800;
const MAX_BYTES = 500 * 1024;

/**
 * Reduce la foto a 800 px de lado y la pasa a WebP (o JPEG si el navegador no
 * sabe). Baja la calidad hasta que pese menos de 500 KB.
 */
export async function preparePhoto(file: File): Promise<string> {
  if (!file.type.startsWith("image/")) throw new Error("Elegí una imagen (JPG, PNG o WebP).");
  const bitmap = await createImageBitmap(file);
  const { sx, sy, sw, sh } = trimBox(bitmap);
  const scale = Math.min(1, MAX_SIDE / Math.max(sw, sh));
  const w = Math.round(sw * scale);
  const h = Math.round(sh * scale);
  const canvas = document.createElement("canvas");
  canvas.width = w;
  canvas.height = h;
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("Este navegador no puede preparar la foto.");
  ctx.fillStyle = "#fff";
  ctx.fillRect(0, 0, w, h);
  ctx.drawImage(bitmap, sx, sy, sw, sh, 0, 0, w, h);
  bitmap.close();
  for (const quality of [0.86, 0.76, 0.64, 0.5]) {
    let url = canvas.toDataURL("image/webp", quality);
    if (!url.startsWith("data:image/webp")) url = canvas.toDataURL("image/jpeg", quality);
    if (url.length * 0.75 < MAX_BYTES) return url;
  }
  throw new Error("La foto es muy pesada. Probá con una más chica.");
}

/**
 * Si la foto tiene fondo blanco (o transparente), recorta el margen sobrante
 * para que el producto llene la tarjeta. Deja un 6 % de aire alrededor.
 */
function trimBox(bitmap: ImageBitmap): { sx: number; sy: number; sw: number; sh: number } {
  const full = { sx: 0, sy: 0, sw: bitmap.width, sh: bitmap.height };
  const step = Math.max(1, Math.round(Math.max(bitmap.width, bitmap.height) / 400));
  const w = Math.ceil(bitmap.width / step);
  const h = Math.ceil(bitmap.height / step);
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  const ctx = c.getContext("2d", { willReadFrequently: true });
  if (!ctx) return full;
  ctx.drawImage(bitmap, 0, 0, w, h);
  const px = ctx.getImageData(0, 0, w, h).data;
  const blank = (i: number) => px[i + 3] < 16 || (px[i] > 240 && px[i + 1] > 240 && px[i + 2] > 240);
  // Solo si las cuatro esquinas son fondo claro.
  const corners = [0, (w - 1) * 4, (h - 1) * w * 4, ((h - 1) * w + w - 1) * 4];
  if (!corners.every(blank)) return full;
  let x0 = w, y0 = h, x1 = -1, y1 = -1;
  for (let y = 0; y < h; y++) {
    for (let x = 0; x < w; x++) {
      if (!blank((y * w + x) * 4)) {
        if (x < x0) x0 = x;
        if (x > x1) x1 = x;
        if (y < y0) y0 = y;
        if (y > y1) y1 = y;
      }
    }
  }
  if (x1 < 0) return full;
  const pad = Math.round(Math.max(x1 - x0, y1 - y0) * 0.06);
  const sx = Math.max(0, (x0 - pad) * step);
  const sy = Math.max(0, (y0 - pad) * step);
  const ex = Math.min(bitmap.width, (x1 + 1 + pad) * step);
  const ey = Math.min(bitmap.height, (y1 + 1 + pad) * step);
  return { sx, sy, sw: ex - sx, sh: ey - sy };
}
