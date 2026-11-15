import { readFile } from "node:fs/promises";
import path from "node:path";

// Sirve los archivos exportados de Stitch (design/stitch) como referencia visual.
const ROOT = path.join(process.cwd(), "design", "stitch");

const CONTENT_TYPES: Record<string, string> = {
  ".html": "text/html; charset=utf-8",
  ".png": "image/png",
  ".json": "application/json; charset=utf-8",
};

export async function GET(
  _request: Request,
  { params }: { params: Promise<{ path: string[] }> },
) {
  // Referencias de diseño: solo en desarrollo, nunca en el dominio publicado.
  if (process.env.NODE_ENV === "production") {
    return new Response("Not found", { status: 404 });
  }
  const { path: segments } = await params;
  const file = path.join(ROOT, ...segments);
  const type = CONTENT_TYPES[path.extname(file)];

  // Evita salir de design/stitch (../) y servir tipos no esperados.
  if (!file.startsWith(ROOT + path.sep) || !type) {
    return new Response("Not found", { status: 404 });
  }

  try {
    const body = await readFile(file);
    return new Response(body, { headers: { "content-type": type } });
  } catch {
    return new Response("Not found", { status: 404 });
  }
}
