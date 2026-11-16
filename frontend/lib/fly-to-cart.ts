/**
 * Animación de "agregar al carrito": una copia de la imagen del producto sale
 * de la tarjeta, sube en arco y cae en el carrito, que rebota al recibirla.
 * El destino es el elemento visible con `data-cart-target` (el carrito en
 * escritorio o la barra inferior en el celular). Con "reducir movimiento"
 * solo rebota el carrito.
 */
export function flyToCart(source: Element | null) {
  if (typeof window === "undefined" || !source) return;
  const target = [...document.querySelectorAll<HTMLElement>("[data-cart-target]")].find((el) => {
    const r = el.getBoundingClientRect();
    return r.width > 0 && r.height > 0;
  });
  if (!target) return;
  const bump = () =>
    target.animate(
      [{ transform: "scale(1)" }, { transform: "scale(1.035)" }, { transform: "scale(1)" }],
      { duration: 320, easing: "cubic-bezier(.3,.7,.3,1.4)" },
    );
  if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) {
    bump();
    return;
  }

  const from = source.getBoundingClientRect();
  const to = target.getBoundingClientRect();
  const toX = to.left + to.width / 2;
  const toY = to.top + Math.min(56, to.height / 2);
  const dx = toX - (from.left + from.width / 2);
  const dy = toY - (from.top + from.height / 2);

  const clone = source.cloneNode(true) as HTMLElement;
  clone.querySelectorAll("[data-fly-skip]").forEach((el) => el.remove());
  Object.assign(clone.style, {
    position: "fixed",
    left: `${from.left}px`,
    top: `${from.top}px`,
    width: `${from.width}px`,
    height: `${from.height}px`,
    margin: "0",
    zIndex: "60",
    pointerEvents: "none",
    borderRadius: "18px",
    willChange: "transform, opacity",
    filter: "drop-shadow(0 18px 24px rgba(20,22,28,.28))",
  });
  clone.setAttribute("aria-hidden", "true");
  document.body.appendChild(clone);

  const lift = Math.min(140, Math.abs(dx) * 0.25 + 60);
  const anim = clone.animate(
    [
      { transform: "translate(0,0) scale(1) rotate(0deg)", opacity: 1 },
      { transform: `translate(${dx * 0.35}px, ${dy * 0.35 - lift}px) scale(.82) rotate(-10deg)`, opacity: 1, offset: 0.42 },
      { transform: `translate(${dx}px, ${dy}px) scale(.16) rotate(8deg)`, opacity: 0.35 },
    ],
    { duration: 640, easing: "cubic-bezier(.45,.05,.3,1)" },
  );
  anim.onfinish = () => {
    clone.remove();
    bump();
  };
  anim.oncancel = () => clone.remove();
}
