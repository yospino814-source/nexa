"use client";

import { useEffect, useRef } from "react";

import { api, COUPON_ERROR_MESSAGES, friendlyError } from "@/lib/api";
import { toSaleItems, useCart } from "@/lib/stores/cart";

/**
 * F1.14: cada cambio del carrito (líneas, cantidades, tarifa, cliente, cupón o
 * descuento) pide `POST /v1/sales/quote` con 300 ms de espera. Solo cuenta la
 * respuesta del último pedido; las anteriores se descartan. Los totales del POS
 * salen exclusivamente de esta cotización.
 */
export function useLiveQuote(branchId: string) {
  const lines = useCart((s) => s.lines);
  const customer = useCart((s) => s.customer);
  const customerAlias = useCart((s) => s.customerAlias);
  const couponCode = useCart((s) => s.couponCode);
  const setQuote = useCart((s) => s.setQuote);
  const setQuoteLoading = useCart((s) => s.setQuoteLoading);
  const setCouponError = useCart((s) => s.setCouponError);
  const setCoupon = useCart((s) => s.setCoupon);
  const lastRequest = useRef(0);

  useEffect(() => {
    const requestId = ++lastRequest.current;
    if (lines.length === 0) {
      setQuote(null);
      return;
    }
    setQuoteLoading(true);
    const timer = setTimeout(async () => {
      try {
        const quote = await api.sales.quote({
          branch_id: branchId,
          items: toSaleItems(lines),
          customer_id: customer?.id ?? null,
          customer_alias: customerAlias ?? null,
          coupon_code: couponCode ?? null,
        });
        if (requestId !== lastRequest.current) return;
        if (quote.coupon_error) {
          // El cupón no aplica: se muestra el motivo y se quita; la cotización ya viene sin él.
          setCouponError(COUPON_ERROR_MESSAGES[quote.coupon_error.code] ?? quote.coupon_error.message);
          setCoupon(null);
        }
        setQuote(quote);
      } catch (e) {
        if (requestId !== lastRequest.current) return;
        setQuote(null, friendlyError(e));
      }
    }, 300);
    return () => clearTimeout(timer);
  }, [branchId, lines, customer, customerAlias, couponCode, setQuote, setQuoteLoading, setCouponError, setCoupon]);
}
