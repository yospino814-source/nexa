/** SVG generado por nuestro backend, mostrado como imagen (sin inyectar HTML). */
export function svgDataUri(svg: string): string {
  return `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`;
}
