#![allow(clippy::type_complexity)]

//! Venta de mostrador en un solo paso (`POST /v1/sales`).
//!
//! Todo ocurre en una transaccion: crear la orden, fijar precios e impuestos
//! del servidor, descontar stock, registrar pagos, asignar el consecutivo del
//! comprobante y encolar el documento fiscal. Si algo falla, no queda nada.
//!
//! **Idempotencia:** el cliente genera `id` (UUID) y lo reusa en cada
//! reintento. Si la venta ya existe con el mismo cuerpo se devuelve tal cual
//! (sin cobrar dos veces); si el cuerpo cambio, se responde 409.

use axum::http::StatusCode;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{self, ApiError, state, validation};
use crate::orders::{self, ITEM_COLS, ITEM_FROM, Order, OrderItem};
use crate::pricing::{self, PriceLine};
use crate::tenancy::{self, Action, Scope};
use crate::{cash, inventory};

type Tx<'a> = Transaction<'a, Postgres>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PaymentMethod {
    Efectivo,
    Datafono,
    Qr,
}

impl PaymentMethod {
    fn as_db(self) -> &'static str {
        match self {
            PaymentMethod::Efectivo => "EFECTIVO",
            PaymentMethod::Datafono => "DATAFONO",
            PaymentMethod::Qr => "QR",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaleItem {
    #[serde(default)]
    pub product_id: Option<Uuid>,
    #[serde(default)]
    pub custom_name: Option<String>,
    pub qty: Decimal,
    #[serde(default)]
    pub unit_price_override: Option<Decimal>,
    #[serde(default)]
    pub discount_pct: Option<Decimal>,
    #[serde(default)]
    pub tier_id: Option<Uuid>,
    #[serde(default)]
    pub price_override_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SalePayment {
    pub method: PaymentMethod,
    /// En efectivo: lo que entrega el cliente (el vuelto lo calcula el
    /// servidor). En los demas metodos: el monto cobrado.
    pub amount: Decimal,
    /// Numero de aprobacion del datafono, referencia de Nequi, etc.
    #[serde(default)]
    pub reference: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SaleRequest {
    /// Clave de idempotencia generada por el cliente; es tambien el id de la orden.
    pub id: Uuid,
    pub branch_id: Uuid,
    pub items: Vec<SaleItem>,
    pub payments: Vec<SalePayment>,
    pub quoted_total: Decimal,
    #[serde(default)]
    pub tip_amount: Option<Decimal>,
    #[serde(default)]
    pub customer_id: Option<Uuid>,
    #[serde(default)]
    pub customer_alias: Option<String>,
    #[serde(default)]
    pub coupon_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteRequest {
    pub branch_id: Uuid,
    pub items: Vec<SaleItem>,
    #[serde(default)]
    pub tip_amount: Option<Decimal>,
    #[serde(default)]
    pub customer_id: Option<Uuid>,
    #[serde(default)]
    pub customer_alias: Option<String>,
    #[serde(default)]
    pub coupon_code: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteLine {
    pub index: usize,
    pub product_id: Option<Uuid>,
    pub name: String,
    pub qty: Decimal,
    pub units_per_package: Decimal,
    pub list_price: Decimal,
    pub unit_price: Decimal,
    pub gross: Decimal,
    pub discount: pricing::LineDiscountBreakdown,
    pub promotion_id: Option<Uuid>,
    pub line_total: Decimal,
    pub tax_rate: Decimal,
    pub tax_amount: Decimal,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuoteResponse {
    pub lines: Vec<QuoteLine>,
    pub subtotal_gross: Decimal,
    pub discount_total: Decimal,
    pub subtotal: Decimal,
    pub tax_total: Decimal,
    pub tip_amount: Decimal,
    pub total: Decimal,
    pub promotions: Vec<pricing::PromoApplied>,
    pub coupon: Option<pricing::CouponApplied>,
    pub coupon_error: Option<pricing::CouponError>,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Payment {
    pub id: Uuid,
    pub method: String,
    pub amount: Decimal,
    pub tendered: Option<Decimal>,
    pub reference: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SaleResponse {
    pub order: Order,
    pub items: Vec<OrderItem>,
    pub payments: Vec<Payment>,
    /// Consecutivo del comprobante (p. ej. `POS-000123`).
    pub receipt_number: String,
    pub change_due: Decimal,
    /// Estado del documento equivalente electronico (`PENDIENTE` al vender).
    pub fiscal_status: String,
}

/// Resultado de un checkout: venta nueva o reintento de una ya registrada.
#[derive(Debug)]
pub enum Checkout {
    Created(SaleResponse),
    Replayed(SaleResponse),
}

/// Validaciones que no necesitan la BD.
fn validate(req: &SaleRequest) -> Result<(), ApiError> {
    if req.items.is_empty() {
        return Err(validation("sale has no items"));
    }
    if req.items.iter().any(|i| i.qty <= Decimal::ZERO) {
        return Err(validation("qty must be > 0"));
    }
    if req.payments.is_empty() {
        return Err(validation("sale has no payments"));
    }
    if req.payments.iter().any(|p| p.amount <= Decimal::ZERO) {
        return Err(validation("payment amount must be > 0"));
    }
    let cash_lines = req
        .payments
        .iter()
        .filter(|p| p.method == PaymentMethod::Efectivo)
        .count();
    if cash_lines > 1 {
        return Err(validation("at most one cash payment per sale"));
    }
    if req.tip_amount.is_some_and(|t| t < Decimal::ZERO) {
        return Err(validation("tip_amount must be >= 0"));
    }
    for item in &req.items {
        if item.unit_price_override.is_some_and(|p| p < Decimal::ZERO) {
            return Err(validation("unit_price_override must be >= 0"));
        }
        if item
            .discount_pct
            .is_some_and(|d| d < Decimal::ZERO || d > Decimal::new(100, 0))
        {
            return Err(validation("discount_pct must be between 0 and 100"));
        }
    }
    Ok(())
}

/// Reparto de los pagos sobre el total: cuanto aplica cada metodo y el vuelto.
#[derive(Debug, PartialEq)]
struct Settlement {
    /// Efectivo aplicado a la orden (lo recibido menos el vuelto).
    cash_applied: Decimal,
    change_due: Decimal,
    paid: Decimal,
}

/// Los pagos no-efectivo se aplican primero y no pueden superar el total (no
/// hay vuelto de una tarjeta); el efectivo cubre el resto y genera el vuelto.
fn settle(total: Decimal, payments: &[SalePayment]) -> Result<Settlement, ApiError> {
    let non_cash: Decimal = payments
        .iter()
        .filter(|p| p.method != PaymentMethod::Efectivo)
        .map(|p| p.amount)
        .sum();
    if non_cash > total {
        return Err(validation("non-cash payments exceed the total"));
    }
    let due = total - non_cash;
    let tendered = payments
        .iter()
        .find(|p| p.method == PaymentMethod::Efectivo)
        .map(|p| p.amount);

    match tendered {
        None if due == Decimal::ZERO => Ok(Settlement {
            cash_applied: Decimal::ZERO,
            change_due: Decimal::ZERO,
            paid: non_cash,
        }),
        None => Err(validation("payments do not cover the total")),
        Some(_) if due == Decimal::ZERO => Err(validation("cash payment is not needed")),
        Some(t) if t < due => Err(validation("payments do not cover the total")),
        Some(t) => Ok(Settlement {
            cash_applied: due,
            change_due: t - due,
            paid: non_cash + t,
        }),
    }
}

/// Registra una venta de mostrador cobrada.
pub async fn checkout(
    pool: &PgPool,
    claims: &Claims,
    req: SaleRequest,
) -> Result<Checkout, ApiError> {
    validate(&req)?;
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, req.branch_id).await?;
    scope.require(Action::Charge)?;
    let result = checkout_tx(&mut tx, &scope, &req).await?;
    tx.commit().await?;
    Ok(result)
}

async fn validate_customer_and_alias(
    tx: &mut Tx<'_>,
    scope: &Scope,
    settings: &crate::settings::BranchSettings,
    customer_id: Option<Uuid>,
    raw_alias: Option<&str>,
) -> Result<(), ApiError> {
    let customer_alias = raw_alias.map(|a| a.trim());

    if customer_id.is_some() && raw_alias.is_some() {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "SALE_CUSTOMER_AND_ALIAS",
            "No se puede especificar cliente y alias al mismo tiempo",
            None,
        ));
    }

    if let Some(alias) = raw_alias {
        let trimmed = alias.trim();
        if trimmed.is_empty() || trimmed.len() > 40 {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "SALE_ALIAS_INVALID",
                "El alias debe tener entre 1 y 40 caracteres",
                None,
            ));
        }
    }

    if customer_alias.is_some() && !settings.pos.allow_alias_sales {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "SALE_ALIAS_NOT_ALLOWED",
            "Las ventas con alias no están permitidas en esta sucursal",
            None,
        ));
    }

    if customer_id.is_none() && customer_alias.is_none() && !settings.pos.allow_anonymous_sales {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "SALE_CUSTOMER_REQUIRED",
            "Esta sucursal requiere identificar al cliente",
            None,
        ));
    }

    if let Some(cid) = customer_id {
        let exists: Option<bool> = sqlx::query_scalar(
            "select true from customers where id = $1 and tenant_id = $2 and archived_at is null",
        )
        .bind(cid)
        .bind(scope.tenant)
        .fetch_optional(&mut **tx)
        .await?;

        if exists.is_none() {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "SALE_CUSTOMER_INVALID",
                "El cliente seleccionado no existe o está archivado",
                None,
            ));
        }
    }

    Ok(())
}

#[derive(Debug, Clone)]
struct PrepItemInfo {
    product_id: Option<Uuid>,
    name: String,
    custom_name: Option<String>,
    tier_id: Option<Uuid>,
    units_per_package: Decimal,
    list_price: Decimal,
    price_override_reason: Option<String>,
}

async fn prepare_pricing_lines(
    tx: &mut Tx<'_>,
    scope: &Scope,
    branch_id: Uuid,
    settings: &crate::settings::BranchSettings,
    items: &[SaleItem],
) -> Result<(Vec<PriceLine>, Vec<PrepItemInfo>), ApiError> {
    let mut price_lines = Vec::with_capacity(items.len());
    let mut prep_infos = Vec::with_capacity(items.len());

    for (index, item) in items.iter().enumerate() {
        if item.qty <= Decimal::ZERO {
            return Err(validation("qty must be > 0"));
        }

        // 1. Descuento manual
        if let Some(disc) = item.discount_pct {
            if disc < Decimal::ZERO || disc > settings.pos.max_discount_pct {
                return Err(error::coded(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "DISCOUNT_ABOVE_MAX",
                    "El descuento supera el máximo permitido para esta sucursal",
                    Some(serde_json::json!({ "max": settings.pos.max_discount_pct })),
                ));
            }
            if disc > Decimal::ZERO {
                scope.require_permission("pos:discount")?;
            }
        }

        // 2. Producto o ítem libre
        match item.product_id {
            None => {
                let custom_name = item
                    .custom_name
                    .as_deref()
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty());
                let custom_name_str = custom_name.ok_or_else(|| {
                    error::coded(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "CUSTOM_ITEM_INVALID",
                        "El ítem libre requiere un nombre de entre 1 y 80 caracteres",
                        None,
                    )
                })?;
                if custom_name_str.len() > 80 {
                    return Err(error::coded(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "CUSTOM_ITEM_INVALID",
                        "El nombre del ítem libre no puede superar 80 caracteres",
                        None,
                    ));
                }

                if !settings.pos.allow_price_override {
                    return Err(error::coded(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "PRICE_OVERRIDE_DISABLED",
                        "Esta sucursal no permite precios personalizados",
                        None,
                    ));
                }
                scope.require_permission("pos:price_override")?;

                let unit_price = item.unit_price_override.ok_or_else(|| {
                    error::coded(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "CUSTOM_ITEM_INVALID",
                        "El ítem libre requiere especificar el precio",
                        None,
                    )
                })?;
                if unit_price <= Decimal::ZERO {
                    return Err(error::coded(
                        StatusCode::UNPROCESSABLE_ENTITY,
                        "CUSTOM_ITEM_INVALID",
                        "El precio del ítem libre debe ser mayor a 0",
                        None,
                    ));
                }

                let unit_price = crate::money::money(unit_price);
                price_lines.push(PriceLine {
                    index,
                    product_id: None,
                    category_id: None,
                    qty: item.qty,
                    unit_price,
                    tier_id: None,
                    overridden: false,
                    custom: true,
                    manual_pct: item.discount_pct,
                    tax_rate: Decimal::ZERO,
                });
                prep_infos.push(PrepItemInfo {
                    product_id: None,
                    name: custom_name_str.to_string(),
                    custom_name: Some(custom_name_str.to_string()),
                    tier_id: None,
                    units_per_package: Decimal::ONE,
                    list_price: unit_price,
                    price_override_reason: item.price_override_reason.clone(),
                });
            }
            Some(pid) => {
                let row: Option<(String, Option<Uuid>, Option<Decimal>, Option<String>)> =
                    sqlx::query_as(
                        "select p.name, p.category_id, coalesce(bp.price, p.base_price) as price, tx.tima_codigo
                           from branch_products bp
                           join products p on p.id = bp.product_id
                           left join tbl_tipos_maestra tx on tx.tima_id = p.tax_type_id
                          where bp.branch_id = $1 and bp.product_id = $2
                            and bp.is_available and p.active",
                    )
                    .bind(branch_id)
                    .bind(pid)
                    .fetch_optional(&mut **tx)
                    .await?;

                let (name, category_id, catalog_price_opt, tax_code) = row.ok_or_else(|| {
                    validation(format!("product {pid} is not in the branch assortment"))
                })?;
                let catalog_price = catalog_price_opt
                    .ok_or_else(|| validation(format!("product {pid} has no base price")))?;
                let tax_rate = crate::catalog::tax_rate_for(tax_code.as_deref());

                let (unit_price, overridden) = if let Some(ov) = item.unit_price_override {
                    if !settings.pos.allow_price_override {
                        return Err(validation("price override is not allowed in this branch"));
                    }
                    scope.require_permission("pos:price_override")?;
                    (crate::money::money(ov), true)
                } else {
                    (catalog_price, false)
                };

                let units_per_package = Decimal::ONE;
                let list_price = catalog_price * units_per_package;

                price_lines.push(PriceLine {
                    index,
                    product_id: Some(pid),
                    category_id,
                    qty: item.qty,
                    unit_price,
                    tier_id: item.tier_id,
                    overridden,
                    custom: false,
                    manual_pct: item.discount_pct,
                    tax_rate: tax_rate.unwrap_or(Decimal::ZERO),
                });
                prep_infos.push(PrepItemInfo {
                    product_id: Some(pid),
                    name,
                    custom_name: None,
                    tier_id: item.tier_id,
                    units_per_package,
                    list_price,
                    price_override_reason: item.price_override_reason.clone(),
                });
            }
        }
    }

    Ok((price_lines, prep_infos))
}

pub async fn checkout_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    req: &SaleRequest,
) -> Result<Checkout, ApiError> {
    let body = serde_json::to_value(req).map_err(anyhow::Error::from)?;
    let branch = req.branch_id;

    // 1. Reclamar el id. Si otro request con el mismo id esta en curso, este
    //    insert espera a que termine y luego no inserta nada.
    let claimed: Option<Uuid> = sqlx::query_scalar(
        "insert into orders (id, tenant_id, branch_id, sale_type_id, status_id, client_request)
         values ($1, $2, $3, public.tipo_id('VENTA', 'MOSTRADOR'), public.tipo_id('ORD', 'CREADA'), $4)
         on conflict (id) do nothing
         returning id",
    )
    .bind(req.id)
    .bind(scope.tenant)
    .bind(branch)
    .bind(&body)
    .fetch_optional(&mut **tx)
    .await?;

    if claimed.is_none() {
        let stored: Option<Option<serde_json::Value>> =
            sqlx::query_scalar("select client_request from orders where id = $1")
                .bind(req.id)
                .fetch_optional(&mut **tx)
                .await?;
        return match stored {
            Some(Some(previous)) if previous == body => {
                Ok(Checkout::Replayed(load_response(tx, req.id).await?))
            }
            _ => Err(state("sale id already used with a different request")),
        };
    }

    // Cargar ajustes de la sucursal para validar reglas de venta y caja
    let settings = crate::settings::load_tx(tx, branch).await?;

    // Validar cliente y alias (F1.4)
    validate_customer_and_alias(
        tx,
        scope,
        &settings,
        req.customer_id,
        req.customer_alias.as_deref(),
    )
    .await?;

    let customer_id = req.customer_id;
    let customer_alias = req.customer_alias.as_deref().map(|a| a.trim());

    // 2. Turno de caja del cajero (si la sucursal lo exige)
    let session = if settings.cash.require_session {
        let s = cash::open_session_tx(tx, scope, branch)
            .await?
            .ok_or_else(|| state("no open cash session for this cashier"))?;
        Some(s)
    } else {
        cash::open_session_tx(tx, scope, branch).await?
    };

    // 5. Cargar contexto de precios y validar
    let (price_lines, prep_infos) =
        prepare_pricing_lines(tx, scope, branch, &settings, &req.items).await?;

    let promos = Vec::new();
    let tip = req.tip_amount.unwrap_or(Decimal::ZERO);

    let priced_cart = match pricing::price_cart(&price_lines, &promos, None, tip) {
        Ok(pc) => pc,
        Err(err) => {
            let code = match err.code.as_str() {
                "COUPON_NOT_APPLICABLE" => "COUPON_NOT_APPLICABLE",
                "COUPON_MIN_SUBTOTAL" => "COUPON_MIN_SUBTOTAL",
                _ => "COUPON_INVALID",
            };
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                code,
                err.message,
                None,
            ));
        }
    };

    // 8. Si money(quoted_total) != total: 409 PRICE_CHANGED
    if crate::money::money(req.quoted_total) != priced_cart.total {
        let quote_lines: Vec<QuoteLine> = priced_cart
            .lines
            .iter()
            .enumerate()
            .map(|(i, pl)| QuoteLine {
                index: i,
                product_id: prep_infos[i].product_id,
                name: prep_infos[i].name.clone(),
                qty: price_lines[i].qty,
                units_per_package: prep_infos[i].units_per_package,
                list_price: prep_infos[i].list_price,
                unit_price: price_lines[i].unit_price,
                gross: pl.gross,
                discount: pl.discount.clone(),
                promotion_id: pl.promotion_id,
                line_total: pl.line_total,
                tax_rate: pl.tax_rate,
                tax_amount: pl.tax_amount,
            })
            .collect();
        let quote_resp = QuoteResponse {
            lines: quote_lines,
            subtotal_gross: priced_cart.subtotal_gross,
            discount_total: priced_cart.discount_total,
            subtotal: priced_cart.subtotal,
            tax_total: priced_cart.tax_total,
            tip_amount: priced_cart.tip_amount,
            total: priced_cart.total,
            promotions: priced_cart.promotions,
            coupon: priced_cart.coupon,
            coupon_error: priced_cart.coupon_error,
        };
        return Err(error::coded(
            StatusCode::CONFLICT,
            "PRICE_CHANGED",
            "Los precios cambiaron. Revisá el total.",
            Some(serde_json::to_value(&quote_resp).unwrap()),
        ));
    }

    // 9. Insertar líneas con todas las columnas
    for (i, pl) in priced_cart.lines.iter().enumerate() {
        sqlx::query(
            "insert into order_items
               (tenant_id, branch_id, order_id, product_id, custom_name, tier_id, units_per_package,
                qty, unit_price, list_price, discount_manual, discount_promotion, discount_order,
                discount_coupon, promotion_id, price_override_reason, tax_rate)
             values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17)",
        )
        .bind(scope.tenant)
        .bind(branch)
        .bind(req.id)
        .bind(prep_infos[i].product_id)
        .bind(&prep_infos[i].custom_name)
        .bind(prep_infos[i].tier_id)
        .bind(prep_infos[i].units_per_package)
        .bind(price_lines[i].qty)
        .bind(price_lines[i].unit_price)
        .bind(prep_infos[i].list_price)
        .bind(pl.discount.manual)
        .bind(pl.discount.promotion)
        .bind(pl.discount.order)
        .bind(pl.discount.coupon)
        .bind(pl.promotion_id)
        .bind(&prep_infos[i].price_override_reason)
        .bind(price_lines[i].tax_rate)
        .execute(&mut **tx)
        .await?;
    }

    // 10. Stock: descuento directo, solo productos
    let stock_lines: Vec<(Uuid, Decimal)> = prep_infos
        .iter()
        .zip(&price_lines)
        .filter_map(|(prep, pl)| {
            prep.product_id
                .map(|pid| (pid, pl.qty * prep.units_per_package))
        })
        .collect();
    if !stock_lines.is_empty() {
        inventory::sell_tx(tx, scope, branch, req.id, &stock_lines).await?;
    }

    // 11. Totales y propina
    sqlx::query(
        "update orders
            set tip_amount = $2,
                customer_id = $3,
                customer_alias = $4
          where id = $1",
    )
    .bind(req.id)
    .bind(tip)
    .bind(customer_id)
    .bind(customer_alias)
    .execute(&mut **tx)
    .await?;
    orders::recalc_totals(tx, req.id).await?;

    let total = priced_cart.total;

    // 6. Pagos.
    let settlement = settle(total, &req.payments)?;
    for p in &req.payments {
        let (amount, tendered) = if p.method == PaymentMethod::Efectivo {
            (settlement.cash_applied, Some(p.amount))
        } else {
            (p.amount, None)
        };
        sqlx::query(
            "insert into payments
               (tenant_id, branch_id, order_id, cash_session_id, method_id, amount, tendered, reference, created_by)
             values ($1, $2, $3, $4, public.tipo_id('PAGO', $5), $6, $7, $8, $9)",
        )
        .bind(scope.tenant)
        .bind(branch)
        .bind(req.id)
        .bind(session)
        .bind(p.method.as_db())
        .bind(amount)
        .bind(tendered)
        .bind(&p.reference)
        .bind(scope.user)
        .execute(&mut **tx)
        .await?;
    }

    // 7. Consecutivo sin huecos + documento fiscal en la outbox.
    let (prefix, number) = crate::sequences::next_tx(tx, scope, branch, "POS").await?;
    sqlx::query(
        "insert into fiscal_documents (tenant_id, branch_id, order_id, doc_type, prefix, number, status_id)
         values ($1, $2, $3, 'POS', $4, $5, public.tipo_id('DOCF', 'PENDIENTE'))",
    )
    .bind(scope.tenant)
    .bind(branch)
    .bind(req.id)
    .bind(&prefix)
    .bind(number)
    .execute(&mut **tx)
    .await?;

    // 8. Cierre de la venta.
    sqlx::query(
        "update orders
            set status_id = public.tipo_id('ORD', 'COBRADA'),
                cash_session_id = $2, receipt_prefix = $3, receipt_number = $4,
                paid_amount = $5, change_due = $6, waiter_id = null
          where id = $1",
    )
    .bind(req.id)
    .bind(session)
    .bind(&prefix)
    .bind(number)
    .bind(settlement.paid)
    .bind(settlement.change_due)
    .execute(&mut **tx)
    .await?;

    Ok(Checkout::Created(load_response(tx, req.id).await?))
}

/// Cotiza una venta calculando descuentos y totales sin tocar inventario ni caja.
pub async fn quote(
    pool: &PgPool,
    claims: &Claims,
    req: QuoteRequest,
) -> Result<QuoteResponse, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, req.branch_id).await?;
    scope.require_permission("pos:sell")?;
    let resp = quote_tx(&mut tx, &scope, req.branch_id, req).await?;
    tx.commit().await?;
    Ok(resp)
}

pub async fn quote_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    branch_id: Uuid,
    req: QuoteRequest,
) -> Result<QuoteResponse, ApiError> {
    let settings = crate::settings::load_tx(tx, branch_id).await?;

    validate_customer_and_alias(
        tx,
        scope,
        &settings,
        req.customer_id,
        req.customer_alias.as_deref(),
    )
    .await?;

    let (price_lines, prep_infos) =
        prepare_pricing_lines(tx, scope, branch_id, &settings, &req.items).await?;

    let promos = Vec::new();
    let tip = req.tip_amount.unwrap_or(Decimal::ZERO);

    let priced_cart = match pricing::price_cart(&price_lines, &promos, None, tip) {
        Ok(pc) => pc,
        Err(err) => {
            let mut pc = pricing::price_cart(&price_lines, &promos, None, tip).unwrap();
            pc.coupon_error = Some(err);
            pc
        }
    };

    let quote_lines: Vec<QuoteLine> = priced_cart
        .lines
        .iter()
        .enumerate()
        .map(|(i, pl)| QuoteLine {
            index: i,
            product_id: prep_infos[i].product_id,
            name: prep_infos[i].name.clone(),
            qty: price_lines[i].qty,
            units_per_package: prep_infos[i].units_per_package,
            list_price: prep_infos[i].list_price,
            unit_price: price_lines[i].unit_price,
            gross: pl.gross,
            discount: pl.discount.clone(),
            promotion_id: pl.promotion_id,
            line_total: pl.line_total,
            tax_rate: pl.tax_rate,
            tax_amount: pl.tax_amount,
        })
        .collect();

    Ok(QuoteResponse {
        lines: quote_lines,
        subtotal_gross: priced_cart.subtotal_gross,
        discount_total: priced_cart.discount_total,
        subtotal: priced_cart.subtotal,
        tax_total: priced_cart.tax_total,
        tip_amount: priced_cart.tip_amount,
        total: priced_cart.total,
        promotions: priced_cart.promotions,
        coupon: priced_cart.coupon,
        coupon_error: priced_cart.coupon_error,
    })
}

fn format_receipt(prefix: &str, number: i64) -> String {
    format!("{prefix}-{number:06}")
}

/// Arma la respuesta desde la BD (la misma para la venta y sus reintentos).
async fn load_response(tx: &mut Tx<'_>, order_id: Uuid) -> Result<SaleResponse, ApiError> {
    let order = orders::order_tx(tx, order_id).await?;
    let items = sqlx::query_as::<_, OrderItem>(&format!(
        "select {ITEM_COLS} from {ITEM_FROM} where oi.order_id = $1 order by oi.created_at, oi.id"
    ))
    .bind(order_id)
    .fetch_all(&mut **tx)
    .await?;
    let payments = sqlx::query_as::<_, Payment>(
        "select p.id, t.tima_codigo as method, p.amount, p.tendered, p.reference
           from payments p
           join tbl_tipos_maestra t on t.tima_id = p.method_id
          where p.order_id = $1
          order by p.created_at, p.id",
    )
    .bind(order_id)
    .fetch_all(&mut **tx)
    .await?;
    let (prefix, number, change_due, fiscal_status): (String, i64, Decimal, String) =
        sqlx::query_as(
            "select o.receipt_prefix, o.receipt_number, o.change_due, t.tima_codigo
               from orders o
               join fiscal_documents f on f.order_id = o.id and f.doc_type = 'POS'
               join tbl_tipos_maestra t on t.tima_id = f.status_id
              where o.id = $1",
        )
        .bind(order_id)
        .fetch_one(&mut **tx)
        .await?;

    Ok(SaleResponse {
        order,
        items,
        payments,
        receipt_number: format_receipt(&prefix, number),
        change_due,
        fiscal_status,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pay(method: PaymentMethod, amount: i64) -> SalePayment {
        SalePayment {
            method,
            amount: Decimal::from(amount),
            reference: None,
        }
    }

    #[test]
    fn settle_cash_with_change() {
        let s = settle(Decimal::from(8950), &[pay(PaymentMethod::Efectivo, 10000)]).unwrap();
        assert_eq!(s.cash_applied, Decimal::from(8950));
        assert_eq!(s.change_due, Decimal::from(1050));
        assert_eq!(s.paid, Decimal::from(10000));
    }

    #[test]
    fn settle_mixed_card_and_cash() {
        let s = settle(
            Decimal::from(8950),
            &[
                pay(PaymentMethod::Datafono, 5000),
                pay(PaymentMethod::Efectivo, 5000),
            ],
        )
        .unwrap();
        assert_eq!(s.cash_applied, Decimal::from(3950));
        assert_eq!(s.change_due, Decimal::from(1050));
    }

    #[test]
    fn settle_rejects_bad_combinations() {
        let total = Decimal::from(1000);
        // Tarjeta por encima del total: no hay vuelto de tarjeta.
        assert!(settle(total, &[pay(PaymentMethod::Datafono, 1500)]).is_err());
        // No alcanza.
        assert!(settle(total, &[pay(PaymentMethod::Qr, 500)]).is_err());
        assert!(settle(total, &[pay(PaymentMethod::Efectivo, 500)]).is_err());
        // Efectivo sobrante cuando la tarjeta ya cubrio todo.
        assert!(
            settle(
                total,
                &[
                    pay(PaymentMethod::Datafono, 1000),
                    pay(PaymentMethod::Efectivo, 100)
                ]
            )
            .is_err()
        );
        // Exacto con QR.
        assert!(settle(total, &[pay(PaymentMethod::Qr, 1000)]).is_ok());
    }

    #[test]
    fn validate_rejects_empty_and_duplicated_cash() {
        let base = SaleRequest {
            id: Uuid::new_v4(),
            branch_id: Uuid::new_v4(),
            items: vec![SaleItem {
                product_id: Some(Uuid::new_v4()),
                custom_name: None,
                qty: Decimal::ONE,
                unit_price_override: None,
                discount_pct: None,
                tier_id: None,
                price_override_reason: None,
            }],
            payments: vec![pay(PaymentMethod::Efectivo, 1000)],
            quoted_total: Decimal::from(1000),
            tip_amount: None,
            customer_id: None,
            customer_alias: None,
            coupon_code: None,
        };
        assert!(validate(&base).is_ok());

        let mut no_items = base.clone();
        no_items.items.clear();
        assert!(validate(&no_items).is_err());

        let mut two_cash = base.clone();
        two_cash.payments.push(pay(PaymentMethod::Efectivo, 1));
        assert!(validate(&two_cash).is_err());

        let mut zero_qty = base;
        zero_qty.items[0].qty = Decimal::ZERO;
        assert!(validate(&zero_qty).is_err());
    }

    #[test]
    fn receipt_format_pads_number() {
        assert_eq!(format_receipt("POS", 123), "POS-000123");
    }
}
