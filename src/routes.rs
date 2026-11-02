//! Endpoints HTTP. Los handlers solo traducen HTTP <-> dominio; las reglas de
//! negocio, permisos y transacciones viven en cada modulo.

use axum::extract::rejection::{JsonRejection, PathRejection, QueryRejection};
use axum::extract::{FromRequest, FromRequestParts, State};
use axum::http::StatusCode;
use axum::routing::{get, post, put};
use axum::{Json, Router};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{self, ApiError, DomainError};
use crate::orders::OrderStatus;
use crate::sales::{Checkout, SaleRequest};
use crate::{
    AppState, audit, cash, catalog, customers, expenses, inventory, locations, options, orders,
    payables, permissions, procurement, products, refunds, registers, sales, sales_history,
    settings, tenancy, users,
};

// Extractores que rechazan entradas invalidas con el mismo formato
// `{ "error": .. }` que el resto de la API (los de axum responden texto plano).

/// Cuerpo JSON.
#[derive(FromRequest)]
#[from_request(via(axum::Json), rejection(ApiError))]
pub struct Body<T>(pub T);

/// Query string.
#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Query), rejection(ApiError))]
pub struct Query<T>(pub T);

/// Parametros de la ruta.
#[derive(FromRequestParts)]
#[from_request(via(axum::extract::Path), rejection(ApiError))]
pub struct Path<T>(pub T);

impl From<JsonRejection> for ApiError {
    fn from(rejection: JsonRejection) -> Self {
        ApiError::Domain(DomainError::Validation(rejection.body_text()))
    }
}

impl From<QueryRejection> for ApiError {
    fn from(rejection: QueryRejection) -> Self {
        ApiError::Domain(DomainError::Validation(rejection.body_text()))
    }
}

impl From<PathRejection> for ApiError {
    fn from(rejection: PathRejection) -> Self {
        ApiError::Domain(DomainError::Validation(rejection.body_text()))
    }
}

type ApiResult<T> = Result<T, ApiError>;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/health", get(health))
        .route("/v1/health/db", get(health_db))
        .route("/v1/context", get(context))
        .route("/v1/branches", get(branches))
        // Catalogo
        .route("/v1/catalog", get(catalog_list))
        .route("/v1/products/{id}/recipe", get(recipe))
        // Importación CSV (F2.6)
        .route("/v1/products/import/analyze", post(import_analyze))
        .route("/v1/products/import/commit", post(import_commit))
        // Productos (F2.3 / F2.5)
        .route("/v1/products", get(list_products).post(create_product))
        .route("/v1/products/{id}", get(get_product).put(update_product))
        .route("/v1/products/{id}/variants", post(create_variant))
        .route("/v1/products/{id}/archive", post(archive_product))
        .route("/v1/products/{id}/unarchive", post(unarchive_product))
        .route("/v1/products/{id}/generate-barcode", post(generate_barcode))
        .route(
            "/v1/branch-products/{product_id}",
            put(update_branch_product),
        )
        // Modificadores (F2.4)
        .route(
            "/v1/option-groups",
            get(list_option_groups).post(create_option_group),
        )
        .route("/v1/option-groups/{id}", put(update_option_group))
        .route(
            "/v1/products/{id}/option-groups",
            put(set_product_option_groups),
        )
        // Clientes
        .route("/v1/customers", get(search_customers).post(create_customer))
        // Tarifas y presentaciones
        .route(
            "/v1/price-tiers",
            get(list_price_tiers).post(create_price_tier),
        )
        .route("/v1/price-tiers/{id}", put(update_price_tier))
        .route(
            "/v1/products/{id}/tier-prices",
            get(get_product_tier_prices).put(set_product_tier_prices),
        )
        // Promociones
        .route(
            "/v1/promotions",
            get(list_promotions).post(create_promotion),
        )
        .route("/v1/promotions/{id}", put(update_promotion))
        .route("/v1/promotions/{id}/activate", post(activate_promotion))
        .route("/v1/promotions/{id}/pause", post(pause_promotion))
        // Cupones
        .route("/v1/coupons", get(list_coupons).post(create_coupon))
        .route("/v1/coupons/{id}", put(update_coupon))
        .route("/v1/coupons/{id}/toggle", post(toggle_coupon))
        // Venta de mostrador y caja
        .route("/v1/sales", get(sales_history_list).post(create_sale))
        .route("/v1/sales/{id}", get(sales_history_detail))
        .route("/v1/sales/quote", post(sales_quote))
        .route("/v1/sales/{id}/void", post(void_sale))
        .route(
            "/v1/cash-registers",
            get(list_cash_registers).post(create_cash_register),
        )
        .route("/v1/cash-registers/{id}", put(update_cash_register))
        .route(
            "/v1/cash-sessions",
            get(cash_history).post(open_cash_session),
        )
        .route("/v1/cash-sessions/current", get(current_cash_session))
        .route("/v1/cash-sessions/{id}/close", post(close_cash_session))
        .route("/v1/cash-sessions/{id}/movements", post(add_cash_movement))
        .route("/v1/cash-sessions/{id}/activity", get(cash_activity))
        // Ordenes (mesa, domicilio, mayorista)
        .route("/v1/orders", post(create_order))
        .route("/v1/orders/{id}", get(get_order))
        .route("/v1/orders/{id}/lines", post(add_order_line))
        .route("/v1/orders/{id}/confirm", post(confirm_order))
        .route("/v1/orders/{id}/transition", post(transition_order))
        .route("/v1/orders/{id}/cancel", post(cancel_order))
        .route("/v1/orders/{id}/refunds", get(order_refunds))
        .route("/v1/refunds", get(list_refunds).post(create_refund))
        .route("/v1/refunds/{id}/approve", post(approve_refund))
        .route("/v1/refunds/{id}/reject", post(reject_refund))
        // Inventario
        .route("/v1/inventory/available", get(available))
        .route("/v1/inventory/movements", get(movements))
        .route("/v1/inventory/replenish", post(replenish))
        .route("/v1/inventory/adjust", post(adjust))
        .route("/v1/inventory/count", post(count))
        .route("/v1/inventory/transfer", post(transfer))
        .route("/v1/inventory/stock", get(stock_levels))
        .route("/v1/inventory/alerts", get(inventory_alerts))
        .route("/v1/inventory/min-qty", put(set_min_qty))
        // Bodegas / Ubicaciones (F2.8)
        .route("/v1/locations", get(list_locations).post(create_location))
        .route("/v1/locations/{id}", put(update_location))
        // Proveedores
        .route("/v1/suppliers", get(list_suppliers).post(create_supplier))
        .route("/v1/suppliers/{id}", get(get_supplier).put(update_supplier))
        .route("/v1/suppliers/{id}/archive", post(archive_supplier))
        .route("/v1/suppliers/{id}/products", put(set_supplier_products))
        // Compras
        .route("/v1/purchases", get(list_purchases).post(create_purchase))
        .route("/v1/purchases/summary", get(purchases_summary))
        .route("/v1/purchases/cost-preview", post(cost_preview_purchase))
        .route("/v1/purchases/{id}", get(get_purchase).put(update_purchase))
        .route("/v1/purchases/{id}/approve", post(approve_purchase))
        .route("/v1/purchases/{id}/receive", post(receive_purchase))
        .route("/v1/purchases/{id}/cancel", post(cancel_purchase))
        // Cuentas por pagar (F2.13)
        .route("/v1/payables", get(list_payables))
        .route("/v1/payables/aging", get(aging_payables))
        .route("/v1/payables/{id}", get(get_payable))
        .route("/v1/payables/{id}/payments", post(create_payable_payment))
        .route("/v1/payables/{id}/write-off", post(write_off_payable))
        // Gastos (F2.14)
        .route(
            "/v1/expense-categories",
            get(list_expense_categories).post(create_expense_category),
        )
        .route("/v1/expense-categories/{id}", put(update_expense_category))
        .route("/v1/expenses", get(list_expenses).post(create_expense))
        .route("/v1/expenses/summary", get(get_expenses_summary))
        .route("/v1/expenses/{id}", get(get_expense))
        .route("/v1/expenses/{id}/approve", post(approve_expense))
        .route("/v1/expenses/{id}/reject", post(reject_expense))
        .route("/v1/expenses/{id}/cancel", post(cancel_expense))
        // Permisos y roles
        .route("/v1/permissions", get(list_permissions))
        .route(
            "/v1/roles/{code}/permissions",
            get(get_role_permissions).put(update_role_permissions),
        )
        // Ajustes
        .route("/v1/settings", get(get_settings).put(update_settings))
        // Auditoria
        .route("/v1/audit", get(list_audit))
        // Usuarios y roles
        .route("/v1/users", get(list_users))
        .route("/v1/waiters", get(list_waiters))
        .route("/v1/business", get(business_info))
        .route("/v1/users/invite", post(invite_user))
        .route("/v1/users/{id}/role", put(update_user_role))
        .route("/v1/users/{id}/status", put(update_user_status))
        // Auth / seguridad
        .route("/v1/auth/login-attempt", post(auth_login_attempt))
}

#[derive(Deserialize)]
struct BranchQuery {
    branch_id: Uuid,
}

// -------------------------------------------------------------------- salud

async fn health() -> Json<serde_json::Value> {
    Json(json!({ "status": "ok" }))
}

async fn health_db(State(state): State<AppState>) -> ApiResult<Json<serde_json::Value>> {
    let role: String = sqlx::query_scalar("select current_user::text")
        .fetch_one(&state.pool)
        .await?;
    Ok(Json(json!({ "db": "ok", "role": role })))
}

/// Identidad y alcance resueltos por membresia (tenant, sucursal y rol), tal
/// como los aplica RLS. 403 si el usuario no pertenece a ningun negocio.
pub async fn context(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<serde_json::Value>> {
    let (scope, mut tx) = tenancy::begin(&state.pool, &user.claims).await?;
    let (uid, tenant, branch): (Option<Uuid>, Option<Uuid>, Option<Uuid>) =
        sqlx::query_as("select auth.uid(), public.current_tenant_id(), public.current_branch_id()")
            .fetch_one(&mut *tx)
            .await?;
    tx.commit().await?;
    let mut perms: Vec<String> = scope.permissions.into_iter().collect();
    perms.sort();
    Ok(Json(json!({
        "uid": uid,
        "tenant": tenant,
        "branch": branch,
        "role": scope.role,
        "permissions": perms,
    })))
}

/// Sucursales a las que el usuario tiene acceso, con su rol en cada una.
async fn branches(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<tenancy::BranchAccess>>> {
    Ok(Json(tenancy::branches(&state.pool, &user.claims).await?))
}

// ----------------------------------------------------------------- catalogo

async fn catalog_list(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<BranchQuery>,
) -> ApiResult<Json<Vec<catalog::CatalogItem>>> {
    Ok(Json(
        catalog::catalog(&state.pool, &user.claims, q.branch_id).await?,
    ))
}

async fn recipe(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Vec<catalog::RecipeItem>>> {
    Ok(Json(catalog::recipe(&state.pool, &user.claims, id).await?))
}

// --------------------------------------------------------------- productos (F2.3)

async fn list_products(
    State(state): State<AppState>,
    user: AuthUser,
    Query(params): Query<products::ProductListParams>,
) -> ApiResult<Json<products::ProductListResponse>> {
    Ok(Json(
        products::list_products(&state.pool, &user.claims, params).await?,
    ))
}

async fn get_product(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Query(params): Query<products::ProductListParams>,
) -> ApiResult<Json<products::ProductDetail>> {
    Ok(Json(
        products::get_product(&state.pool, &user.claims, id, params.branch_id).await?,
    ))
}

async fn create_product(
    State(state): State<AppState>,
    user: AuthUser,
    Body(input): Body<products::ProductInput>,
) -> ApiResult<(StatusCode, Json<products::ProductDetail>)> {
    let detail = products::create_product(&state.pool, &user.claims, input).await?;
    Ok((StatusCode::CREATED, Json(detail)))
}

async fn update_product(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(input): Body<products::ProductInput>,
) -> ApiResult<Json<products::ProductDetail>> {
    Ok(Json(
        products::update_product(&state.pool, &user.claims, id, input).await?,
    ))
}

async fn create_variant(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(input): Body<products::CreateVariantInput>,
) -> ApiResult<(StatusCode, Json<products::ProductDetail>)> {
    let detail = products::create_variant(&state.pool, &user.claims, id, input).await?;
    Ok((StatusCode::CREATED, Json(detail)))
}

async fn update_branch_product(
    State(state): State<AppState>,
    user: AuthUser,
    Path(product_id): Path<Uuid>,
    Body(input): Body<products::UpdateBranchProductInput>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(
        products::update_branch_product(&state.pool, &user.claims, product_id, input).await?,
    ))
}

async fn archive_product(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(input): Body<products::ArchiveProductInput>,
) -> ApiResult<Json<serde_json::Value>> {
    let res = products::archive_product(&state.pool, &user.claims, id, input.confirm).await?;
    Ok(Json(res))
}

async fn unarchive_product(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let res = products::unarchive_product(&state.pool, &user.claims, id).await?;
    Ok(Json(res))
}

async fn generate_barcode(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    let barcode = products::generate_barcode(&state.pool, &user.claims, id).await?;
    Ok(Json(json!({ "barcode": barcode })))
}

// ----------------------------------------------------------- modificadores (F2.4)

async fn list_option_groups(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<options::OptionGroupDetail>>> {
    Ok(Json(
        options::list_option_groups(&state.pool, &user.claims).await?,
    ))
}

async fn create_option_group(
    State(state): State<AppState>,
    user: AuthUser,
    Body(input): Body<options::CreateOptionGroupInput>,
) -> ApiResult<(StatusCode, Json<options::OptionGroupDetail>)> {
    let detail = options::create_option_group(&state.pool, &user.claims, input).await?;
    Ok((StatusCode::CREATED, Json(detail)))
}

async fn update_option_group(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(input): Body<options::CreateOptionGroupInput>,
) -> ApiResult<Json<options::OptionGroupDetail>> {
    Ok(Json(
        options::update_option_group(&state.pool, &user.claims, id, input).await?,
    ))
}

async fn set_product_option_groups(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(input): Body<Vec<options::UpdateProductOptionGroupInput>>,
) -> ApiResult<Json<serde_json::Value>> {
    Ok(Json(
        options::set_product_option_groups(&state.pool, &user.claims, id, input).await?,
    ))
}

// ------------------------------------------------------------ venta y caja

/// 201 si la venta es nueva; 200 si es un reintento de una ya registrada.
async fn create_sale(
    State(state): State<AppState>,
    user: AuthUser,
    Body(req): Body<SaleRequest>,
) -> ApiResult<(StatusCode, Json<sales::SaleResponse>)> {
    Ok(
        match sales::checkout(&state.pool, &user.claims, req).await? {
            Checkout::Created(r) => (StatusCode::CREATED, Json(r)),
            Checkout::Replayed(r) => (StatusCode::OK, Json(r)),
        },
    )
}

async fn sales_quote(
    State(state): State<AppState>,
    user: AuthUser,
    Body(req): Body<sales::QuoteRequest>,
) -> ApiResult<Json<sales::QuoteResponse>> {
    Ok(Json(sales::quote(&state.pool, &user.claims, req).await?))
}

async fn list_cash_registers(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<BranchQuery>,
) -> ApiResult<Json<Vec<registers::Register>>> {
    Ok(Json(
        registers::list(&state.pool, &user.claims, q.branch_id).await?,
    ))
}

async fn create_cash_register(
    State(state): State<AppState>,
    user: AuthUser,
    Body(req): Body<registers::CreateRegisterRequest>,
) -> ApiResult<(StatusCode, Json<registers::Register>)> {
    let reg = registers::create(&state.pool, &user.claims, req).await?;
    Ok((StatusCode::CREATED, Json(reg)))
}

async fn update_cash_register(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(req): Body<registers::UpdateRegisterRequest>,
) -> ApiResult<Json<registers::Register>> {
    let reg = registers::update(&state.pool, &user.claims, id, req).await?;
    Ok(Json(reg))
}

#[derive(Deserialize)]
struct OpenCashSession {
    branch_id: Uuid,
    register_id: Uuid,
    opening_amount: Decimal,
}

async fn open_cash_session(
    State(state): State<AppState>,
    user: AuthUser,
    Body(req): Body<OpenCashSession>,
) -> ApiResult<(StatusCode, Json<cash::CashSession>)> {
    let session = cash::open(
        &state.pool,
        &user.claims,
        req.branch_id,
        req.register_id,
        req.opening_amount,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(session)))
}

/// Turno abierto del cajero en la sucursal, o `null`.
async fn current_cash_session(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<BranchQuery>,
) -> ApiResult<Json<Option<cash::CashSummary>>> {
    Ok(Json(
        cash::current(&state.pool, &user.claims, q.branch_id).await?,
    ))
}

#[derive(Deserialize)]
struct CloseCashSession {
    counted_cash: Decimal,
    state_token: String,
    #[serde(default)]
    notes: Option<String>,
}

async fn close_cash_session(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(req): Body<CloseCashSession>,
) -> ApiResult<Json<cash::CashSummary>> {
    Ok(Json(
        cash::close(
            &state.pool,
            &user.claims,
            id,
            req.counted_cash,
            req.notes,
            &req.state_token,
        )
        .await?,
    ))
}

#[derive(Deserialize)]
struct CashMovementBody {
    kind: cash::CashMovementKind,
    amount: Decimal,
    #[serde(default)]
    reason: Option<String>,
    #[serde(default)]
    reason_code: Option<String>,
}

/// Ingreso o retiro de efectivo del turno; devuelve el resumen actualizado.
async fn add_cash_movement(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(req): Body<CashMovementBody>,
) -> ApiResult<(StatusCode, Json<cash::CashSummary>)> {
    let summary = cash::add_movement(
        &state.pool,
        &user.claims,
        id,
        req.kind,
        req.amount,
        req.reason.as_deref(),
        req.reason_code.as_deref(),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(summary)))
}

/// Bitacora del turno (apertura, ventas, ingresos, retiros, cierre).
async fn cash_activity(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<cash::ActivityResponse>> {
    Ok(Json(cash::activity(&state.pool, &user.claims, id).await?))
}

#[derive(Deserialize)]
struct HistoryQuery {
    branch_id: Uuid,
    #[serde(default)]
    limit: Option<i64>,
}

/// Turnos de la sucursal (un cajero ve solo los suyos).
async fn cash_history(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<HistoryQuery>,
) -> ApiResult<Json<Vec<cash::SessionRow>>> {
    Ok(Json(
        cash::history(
            &state.pool,
            &user.claims,
            q.branch_id,
            q.limit.unwrap_or(30),
        )
        .await?,
    ))
}

// ------------------------------------------------------------------ ordenes

#[derive(Deserialize)]
struct CreateOrder {
    branch_id: Uuid,
    sale_type: String,
    #[serde(default)]
    table_id: Option<Uuid>,
    #[serde(default)]
    waiter_id: Option<Uuid>,
    #[serde(default)]
    notes: Option<String>,
}

async fn create_order(
    State(state): State<AppState>,
    user: AuthUser,
    Body(req): Body<CreateOrder>,
) -> ApiResult<(StatusCode, Json<orders::Order>)> {
    let order = orders::create_order(
        &state.pool,
        &user.claims,
        req.branch_id,
        &req.sale_type,
        req.table_id,
        req.waiter_id,
        req.notes,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(order)))
}

async fn get_order(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<orders::Order>> {
    Ok(Json(orders::order(&state.pool, &user.claims, id).await?))
}

#[derive(Deserialize)]
struct AddLine {
    product_id: Uuid,
    qty: Decimal,
    #[serde(default)]
    notes: Option<String>,
}

async fn add_order_line(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(req): Body<AddLine>,
) -> ApiResult<(StatusCode, Json<orders::OrderItem>)> {
    let line = orders::add_line(
        &state.pool,
        &user.claims,
        id,
        req.product_id,
        req.qty,
        req.notes,
    )
    .await?;
    Ok((StatusCode::CREATED, Json(line)))
}

async fn confirm_order(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<orders::Order>> {
    orders::confirm(&state.pool, &user.claims, id).await?;
    Ok(Json(orders::order(&state.pool, &user.claims, id).await?))
}

#[derive(Deserialize)]
struct Transition {
    to: OrderStatus,
}

async fn transition_order(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(req): Body<Transition>,
) -> ApiResult<Json<orders::Order>> {
    orders::transition(&state.pool, &user.claims, id, req.to).await?;
    Ok(Json(orders::order(&state.pool, &user.claims, id).await?))
}

async fn cancel_order(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<orders::Order>> {
    orders::cancel(&state.pool, &user.claims, id).await?;
    Ok(Json(orders::order(&state.pool, &user.claims, id).await?))
}

// --------------------------------------------------------------- inventario

#[derive(Deserialize)]
struct AvailableQuery {
    branch_id: Uuid,
    product_id: Uuid,
}

/// Disponible (on_hand - reservado) de un producto; derivado si es compuesto.
async fn available(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<AvailableQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let value = inventory::available(&state.pool, &user.claims, q.branch_id, q.product_id).await?;
    Ok(Json(
        json!({ "product_id": q.product_id, "available": value }),
    ))
}

#[derive(Deserialize)]
struct KardexQuery {
    branch_id: Uuid,
    #[serde(default)]
    product_id: Option<Uuid>,
    #[serde(default)]
    location_id: Option<Uuid>,
    #[serde(default)]
    limit: Option<i64>,
}

/// Kardex de la sucursal, mas reciente primero.
async fn movements(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<KardexQuery>,
) -> ApiResult<Json<Vec<inventory::Movement>>> {
    Ok(Json(
        inventory::kardex(
            &state.pool,
            &user.claims,
            q.branch_id,
            q.product_id,
            q.location_id,
            q.limit.unwrap_or(200),
        )
        .await?,
    ))
}

#[derive(Deserialize)]
struct StockLevelsQuery {
    branch_id: Uuid,
    #[serde(default)]
    location_id: Option<Uuid>,
}

/// Existencias de la sucursal con estado frente al punto de reorden.
async fn stock_levels(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<StockLevelsQuery>,
) -> ApiResult<Json<Vec<inventory::StockLevel>>> {
    Ok(Json(
        inventory::stock_levels(&state.pool, &user.claims, q.branch_id, q.location_id).await?,
    ))
}

#[derive(Deserialize)]
struct AlertsQuery {
    branch_id: Uuid,
}

async fn inventory_alerts(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<AlertsQuery>,
) -> ApiResult<Json<Vec<inventory::StockAlert>>> {
    let alerts = inventory::list_alerts(&state.pool, &user.claims, q.branch_id).await?;
    Ok(Json(alerts))
}

#[derive(Deserialize)]
struct CountBody {
    branch_id: Uuid,
    product_id: Uuid,
    /// Unidades contadas en fisico.
    counted: Decimal,
    cause: String,
}

/// Conteo fisico: fija la existencia y registra la diferencia en el kardex.
async fn count(
    State(state): State<AppState>,
    user: AuthUser,
    Body(req): Body<CountBody>,
) -> ApiResult<Json<serde_json::Value>> {
    let delta = inventory::count(
        &state.pool,
        &user.claims,
        req.branch_id,
        req.product_id,
        req.counted,
        &req.cause,
    )
    .await?;
    Ok(Json(json!({ "delta": delta })))
}

#[derive(Deserialize)]
struct TransferBody {
    #[serde(default)]
    from_branch_id: Option<Uuid>,
    #[serde(default)]
    to_branch_id: Option<Uuid>,
    #[serde(default)]
    from_location_id: Option<Uuid>,
    #[serde(default)]
    to_location_id: Option<Uuid>,
    product_id: Uuid,
    qty: Decimal,
    cause: String,
}

/// Traslado de unidades entre sucursales o entre bodegas del mismo negocio.
async fn transfer(
    State(state): State<AppState>,
    user: AuthUser,
    Body(req): Body<TransferBody>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    let reference = inventory::transfer_locations(
        &state.pool,
        &user.claims,
        inventory::TransferArgs {
            from_branch_id: req.from_branch_id,
            to_branch_id: req.to_branch_id,
            from_location_id: req.from_location_id,
            to_location_id: req.to_location_id,
            product_id: req.product_id,
            qty: req.qty,
            cause: req.cause,
        },
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "transfer_id": reference })),
    ))
}

#[derive(Deserialize)]
struct MinQtyBody {
    branch_id: Uuid,
    product_id: Uuid,
    min_qty: Decimal,
}

/// Punto de reorden de un producto en la sucursal.
async fn set_min_qty(
    State(state): State<AppState>,
    user: AuthUser,
    Body(req): Body<MinQtyBody>,
) -> ApiResult<StatusCode> {
    inventory::set_min_qty(
        &state.pool,
        &user.claims,
        req.branch_id,
        req.product_id,
        req.min_qty,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct LocationListQuery {
    branch_id: Uuid,
}

async fn list_locations(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<LocationListQuery>,
) -> ApiResult<Json<Vec<locations::LocationRow>>> {
    Ok(Json(
        locations::list_locations(&state.pool, &user.claims, q.branch_id).await?,
    ))
}

async fn create_location(
    State(state): State<AppState>,
    user: AuthUser,
    Body(input): Body<locations::CreateLocationInput>,
) -> ApiResult<(StatusCode, Json<locations::LocationRow>)> {
    let loc = locations::create_location(&state.pool, &user.claims, input).await?;
    Ok((StatusCode::CREATED, Json(loc)))
}

async fn update_location(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(input): Body<locations::UpdateLocationInput>,
) -> ApiResult<Json<locations::LocationRow>> {
    Ok(Json(
        locations::update_location(&state.pool, &user.claims, id, input).await?,
    ))
}

#[derive(Deserialize)]
struct StockChange {
    branch_id: Uuid,
    product_id: Uuid,
    /// Cantidad a reponer (> 0) o delta con signo para un ajuste.
    qty: Decimal,
    cause: String,
}

async fn replenish(
    State(state): State<AppState>,
    user: AuthUser,
    Body(req): Body<StockChange>,
) -> ApiResult<StatusCode> {
    inventory::replenish(
        &state.pool,
        &user.claims,
        req.branch_id,
        req.product_id,
        req.qty,
        &req.cause,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn adjust(
    State(state): State<AppState>,
    user: AuthUser,
    Body(req): Body<StockChange>,
) -> ApiResult<StatusCode> {
    inventory::adjust(
        &state.pool,
        &user.claims,
        req.branch_id,
        req.product_id,
        req.qty,
        &req.cause,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

// ---------------------------------------------------------------- proveedores

#[derive(Deserialize, Default)]
struct SuppliersQuery {
    q: Option<String>,
    #[serde(default)]
    include_inactive: Option<bool>,
}

async fn list_suppliers(
    State(state): State<AppState>,
    user: AuthUser,
    Query(query): Query<SuppliersQuery>,
) -> ApiResult<Json<Vec<procurement::Supplier>>> {
    let rows = procurement::list_suppliers(
        &state.pool,
        &user.claims,
        query.q,
        query.include_inactive.unwrap_or(false),
    )
    .await?;
    Ok(Json(rows))
}

async fn create_supplier(
    State(state): State<AppState>,
    user: AuthUser,
    Body(body): Body<procurement::CreateSupplierRequest>,
) -> ApiResult<(StatusCode, Json<procurement::Supplier>)> {
    let supplier = procurement::create_supplier(&state.pool, &user.claims, body).await?;
    Ok((StatusCode::CREATED, Json(supplier)))
}

async fn update_supplier(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(body): Body<procurement::UpdateSupplierRequest>,
) -> ApiResult<Json<procurement::Supplier>> {
    let supplier = procurement::update_supplier(&state.pool, &user.claims, id, body).await?;
    Ok(Json(supplier))
}

async fn get_supplier(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<procurement::SupplierDetailResponse>> {
    let res = procurement::get_supplier(&state.pool, &user.claims, id).await?;
    Ok(Json(res))
}

async fn archive_supplier(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    procurement::archive_supplier(&state.pool, &user.claims, id).await?;
    Ok(StatusCode::OK)
}

async fn set_supplier_products(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(body): Body<Vec<procurement::SupplierProductItemInput>>,
) -> ApiResult<Json<Vec<procurement::SupplierProductDetail>>> {
    let res = procurement::set_supplier_products(&state.pool, &user.claims, id, body).await?;
    Ok(Json(res))
}

// -------------------------------------------------------------------- compras

async fn list_purchases(
    State(state): State<AppState>,
    user: AuthUser,
    Query(query): Query<procurement::PurchaseFilter>,
) -> ApiResult<Json<Vec<procurement::PurchaseRow>>> {
    let rows = procurement::list_purchases(&state.pool, &user.claims, query).await?;
    Ok(Json(rows))
}

async fn create_purchase(
    State(state): State<AppState>,
    user: AuthUser,
    Body(body): Body<procurement::CreatePurchaseRequest>,
) -> ApiResult<(StatusCode, Json<procurement::PurchaseDetail>)> {
    let detail = procurement::create_purchase(&state.pool, &user.claims, body).await?;
    Ok((StatusCode::CREATED, Json(detail)))
}

#[derive(Deserialize, Default)]
struct PurchasesSummaryQuery {
    branch_id: Option<Uuid>,
    month: Option<String>,
}

async fn purchases_summary(
    State(state): State<AppState>,
    user: AuthUser,
    Query(query): Query<PurchasesSummaryQuery>,
) -> ApiResult<Json<procurement::PurchaseSummary>> {
    let summary =
        procurement::purchase_summary(&state.pool, &user.claims, query.branch_id, query.month)
            .await?;
    Ok(Json(summary))
}

async fn get_purchase(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<procurement::PurchaseDetail>> {
    let detail = procurement::get_purchase(&state.pool, &user.claims, id).await?;
    Ok(Json(detail))
}

async fn cost_preview_purchase(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<procurement::CostPreviewRequest>,
) -> ApiResult<Json<procurement::CostPreviewResponse>> {
    let resp = procurement::cost_preview(&state.pool, &user.claims, req).await?;
    Ok(Json(resp))
}

async fn update_purchase(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Json(req): Json<procurement::UpdatePurchaseRequest>,
) -> ApiResult<Json<procurement::PurchaseDetail>> {
    let detail = procurement::update_purchase(&state.pool, &user.claims, id, req).await?;
    Ok(Json(detail))
}

async fn approve_purchase(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<procurement::PurchaseDetail>> {
    let detail = procurement::approve_purchase(&state.pool, &user.claims, id).await?;
    Ok(Json(detail))
}

async fn receive_purchase(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    payload: Option<Json<procurement::ReceivePurchaseRequest>>,
) -> ApiResult<Json<procurement::PurchaseDetail>> {
    let detail = match payload {
        Some(Json(req)) => {
            procurement::receive_purchase_with_req(&state.pool, &user.claims, id, req).await?
        }
        None => procurement::receive_purchase(&state.pool, &user.claims, id).await?,
    };
    Ok(Json(detail))
}

async fn cancel_purchase(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<procurement::PurchaseDetail>> {
    let detail = procurement::cancel_purchase(&state.pool, &user.claims, id).await?;
    Ok(Json(detail))
}

// ----------------------------------------------------------- cuentas por pagar

async fn list_payables(
    State(state): State<AppState>,
    user: AuthUser,
    Query(filter): Query<payables::PayablesFilter>,
) -> ApiResult<Json<Vec<payables::PayableRow>>> {
    let (scope, mut tx) = if let Some(bid) = filter.branch_id {
        tenancy::begin_in_branch(&state.pool, &user.claims, bid).await?
    } else {
        tenancy::begin(&state.pool, &user.claims).await?
    };
    let rows = payables::list_payables_tx(&mut tx, &scope, filter).await?;
    tx.commit().await?;
    Ok(Json(rows))
}

async fn aging_payables(
    State(state): State<AppState>,
    user: AuthUser,
    Query(filter): Query<payables::AgingFilter>,
) -> ApiResult<Json<payables::PayablesAging>> {
    let (scope, mut tx) = if let Some(bid) = filter.branch_id {
        tenancy::begin_in_branch(&state.pool, &user.claims, bid).await?
    } else {
        tenancy::begin(&state.pool, &user.claims).await?
    };
    let row = payables::aging_summary_tx(&mut tx, &scope, filter.branch_id).await?;
    tx.commit().await?;
    Ok(Json(row))
}

async fn get_payable(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<payables::PayableDetail>> {
    let (scope, mut tx) = tenancy::begin(&state.pool, &user.claims).await?;
    let detail = payables::get_payable_detail_tx(&mut tx, &scope, id).await?;
    tx.commit().await?;
    Ok(Json(detail))
}

async fn create_payable_payment(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(body): Body<payables::CreatePayablePaymentRequest>,
) -> ApiResult<Json<payables::PayableDetail>> {
    let (scope, mut tx) = tenancy::begin(&state.pool, &user.claims).await?;
    let detail = payables::create_payment_tx(&mut tx, &scope, id, body).await?;
    tx.commit().await?;
    Ok(Json(detail))
}

async fn write_off_payable(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(body): Body<payables::WriteOffPayableRequest>,
) -> ApiResult<Json<payables::PayableDetail>> {
    let (scope, mut tx) = tenancy::begin(&state.pool, &user.claims).await?;
    let detail = payables::write_off_tx(&mut tx, &scope, id, body).await?;
    tx.commit().await?;
    Ok(Json(detail))
}

// ------------------------------------------------------------------- gastos

async fn list_expense_categories(
    State(state): State<AppState>,
    user: AuthUser,
    Query(filter): Query<expenses::CategoriesFilter>,
) -> ApiResult<Json<Vec<expenses::ExpenseCategory>>> {
    let (scope, mut tx) = tenancy::begin(&state.pool, &user.claims).await?;
    let rows =
        expenses::list_categories_tx(&mut tx, &scope, filter.include_inactive.unwrap_or(false))
            .await?;
    tx.commit().await?;
    Ok(Json(rows))
}

async fn create_expense_category(
    State(state): State<AppState>,
    user: AuthUser,
    Body(body): Body<expenses::CreateExpenseCategoryRequest>,
) -> ApiResult<Json<expenses::ExpenseCategory>> {
    let (scope, mut tx) = tenancy::begin(&state.pool, &user.claims).await?;
    let cat = expenses::create_category_tx(&mut tx, &scope, body).await?;
    tx.commit().await?;
    Ok(Json(cat))
}

async fn update_expense_category(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(body): Body<expenses::UpdateExpenseCategoryRequest>,
) -> ApiResult<Json<expenses::ExpenseCategory>> {
    let (scope, mut tx) = tenancy::begin(&state.pool, &user.claims).await?;
    let cat = expenses::update_category_tx(&mut tx, &scope, id, body).await?;
    tx.commit().await?;
    Ok(Json(cat))
}

async fn list_expenses(
    State(state): State<AppState>,
    user: AuthUser,
    Query(filter): Query<expenses::ExpensesFilter>,
) -> ApiResult<Json<Vec<expenses::ExpenseRow>>> {
    let (scope, mut tx) = if let Some(bid) = filter.branch_id {
        tenancy::begin_in_branch(&state.pool, &user.claims, bid).await?
    } else {
        tenancy::begin(&state.pool, &user.claims).await?
    };
    let rows = expenses::list_expenses_tx(&mut tx, &scope, filter).await?;
    tx.commit().await?;
    Ok(Json(rows))
}

async fn create_expense(
    State(state): State<AppState>,
    user: AuthUser,
    Body(body): Body<expenses::CreateExpenseRequest>,
) -> ApiResult<Json<expenses::ExpenseRow>> {
    let (scope, mut tx) =
        tenancy::begin_in_branch(&state.pool, &user.claims, body.branch_id).await?;
    let row = expenses::create_expense_tx(&mut tx, &scope, body).await?;
    tx.commit().await?;
    Ok(Json(row))
}

async fn get_expenses_summary(
    State(state): State<AppState>,
    user: AuthUser,
    Query(filter): Query<expenses::ExpenseSummaryFilter>,
) -> ApiResult<Json<expenses::ExpenseSummary>> {
    let (scope, mut tx) = if let Some(bid) = filter.branch_id {
        tenancy::begin_in_branch(&state.pool, &user.claims, bid).await?
    } else {
        tenancy::begin(&state.pool, &user.claims).await?
    };
    let summary = expenses::expenses_summary_tx(&mut tx, &scope, filter).await?;
    tx.commit().await?;
    Ok(Json(summary))
}

async fn get_expense(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<expenses::ExpenseRow>> {
    let (scope, mut tx) = tenancy::begin(&state.pool, &user.claims).await?;
    let row = expenses::get_expense_detail_tx(&mut tx, &scope, id).await?;
    tx.commit().await?;
    Ok(Json(row))
}

async fn approve_expense(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<expenses::ExpenseRow>> {
    let (scope, mut tx) = tenancy::begin(&state.pool, &user.claims).await?;
    let row = expenses::approve_expense_tx(&mut tx, &scope, id).await?;
    tx.commit().await?;
    Ok(Json(row))
}

async fn reject_expense(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(body): Body<expenses::RejectExpenseRequest>,
) -> ApiResult<Json<expenses::ExpenseRow>> {
    let (scope, mut tx) = tenancy::begin(&state.pool, &user.claims).await?;
    let row = expenses::reject_expense_tx(&mut tx, &scope, id, body).await?;
    tx.commit().await?;
    Ok(Json(row))
}

async fn cancel_expense(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<expenses::ExpenseRow>> {
    let (scope, mut tx) = tenancy::begin(&state.pool, &user.claims).await?;
    let row = expenses::cancel_expense_tx(&mut tx, &scope, id).await?;
    tx.commit().await?;
    Ok(Json(row))
}

// ------------------------------------------------------------- permisos y roles

async fn list_permissions(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<permissions::Permission>>> {
    let perms = permissions::list_permissions(&state.pool, &user.claims).await?;
    Ok(Json(perms))
}

async fn get_role_permissions(
    State(state): State<AppState>,
    user: AuthUser,
    Path(code): Path<String>,
) -> ApiResult<Json<permissions::RolePermissionsResponse>> {
    let resp = permissions::get_role_permissions(&state.pool, &user.claims, &code).await?;
    Ok(Json(resp))
}

async fn update_role_permissions(
    State(state): State<AppState>,
    user: AuthUser,
    Path(code): Path<String>,
    Body(req): Body<permissions::UpdateRolePermissionsRequest>,
) -> ApiResult<Json<permissions::RolePermissionsResponse>> {
    let resp = permissions::update_role_permissions(&state.pool, &user.claims, &code, req).await?;
    Ok(Json(resp))
}

// -------------------------------------------------------------------- ajustes

async fn get_settings(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<BranchQuery>,
) -> ApiResult<Json<settings::BranchSettings>> {
    let s = settings::get_settings(&state.pool, &user.claims, q.branch_id).await?;
    Ok(Json(s))
}

async fn update_settings(
    State(state): State<AppState>,
    user: AuthUser,
    Body(req): Body<settings::UpdateSettingsRequest>,
) -> ApiResult<Json<settings::BranchSettings>> {
    let s = settings::update_settings(&state.pool, &user.claims, req).await?;
    Ok(Json(s))
}

// ------------------------------------------------------------------ auditoria

async fn list_audit(
    State(state): State<AppState>,
    user: AuthUser,
    Query(filter): Query<audit::AuditFilter>,
) -> ApiResult<Json<Vec<audit::AuditEntry>>> {
    let entries = audit::list_audit_entries(&state.pool, &user.claims, filter).await?;
    Ok(Json(entries))
}

// ------------------------------------------------------------------- usuarios

async fn list_users(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<users::UsersListResponse>> {
    let resp = users::list_users(&state.pool, &user.claims).await?;
    Ok(Json(resp))
}

async fn list_waiters(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<BranchQuery>,
) -> ApiResult<Json<Vec<users::Waiter>>> {
    Ok(Json(
        users::list_waiters(&state.pool, &user.claims, q.branch_id).await?,
    ))
}

async fn invite_user(
    State(state): State<AppState>,
    user: AuthUser,
    Body(req): Body<users::InviteUserRequest>,
) -> ApiResult<(StatusCode, Json<users::Invitation>)> {
    let inv = users::invite_user(&state.pool, &user.claims, req).await?;
    Ok((StatusCode::CREATED, Json(inv)))
}

async fn update_user_role(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(req): Body<users::UpdateUserRoleRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    users::update_user_role(&state.pool, &user.claims, id, req).await?;
    Ok(Json(json!({ "status": "ok" })))
}

async fn update_user_status(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(req): Body<users::UpdateUserStatusRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    users::update_user_status(&state.pool, &user.claims, id, req).await?;
    Ok(Json(json!({ "status": "ok" })))
}

// ---------------------------------------------------------------------- auth / seguridad

#[derive(Debug, Deserialize)]
pub struct LoginAttemptRequest {
    pub email: String,
    pub success: Option<bool>,
    #[serde(alias = "failure_reason")]
    pub reason: Option<String>,
}

async fn auth_login_attempt(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Body(req): Body<LoginAttemptRequest>,
) -> ApiResult<Json<serde_json::Value>> {
    let ip = headers
        .get("x-forwarded-for")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.split(',').next().unwrap_or(s).trim());
    let user_agent = headers.get("user-agent").and_then(|h| h.to_str().ok());

    let count = crate::auth::check_and_record_login_attempt(
        &state.pool,
        &req.email,
        ip,
        user_agent,
        req.success,
        req.reason.as_deref(),
    )
    .await?;

    Ok(Json(
        json!({ "status": "ok", "failed_attempts_in_window": count }),
    ))
}

async fn search_customers(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<customers::CustomerQuery>,
) -> ApiResult<Json<Vec<customers::Customer>>> {
    Ok(Json(customers::search(&state.pool, &user.claims, q).await?))
}

async fn create_customer(
    State(state): State<AppState>,
    user: AuthUser,
    Body(input): Body<customers::CreateCustomerInput>,
) -> ApiResult<(StatusCode, Json<customers::Customer>)> {
    let customer = customers::create(&state.pool, &user.claims, input).await?;
    Ok((StatusCode::CREATED, Json(customer)))
}

async fn list_price_tiers(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<crate::tiers::PriceTier>>> {
    Ok(Json(
        crate::tiers::list_tiers(&state.pool, &user.claims).await?,
    ))
}

async fn create_price_tier(
    State(state): State<AppState>,
    user: AuthUser,
    Body(req): Body<crate::tiers::CreatePriceTierRequest>,
) -> ApiResult<(StatusCode, Json<crate::tiers::PriceTier>)> {
    let tier = crate::tiers::create_tier(&state.pool, &user.claims, req).await?;
    Ok((StatusCode::CREATED, Json(tier)))
}

async fn update_price_tier(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(req): Body<crate::tiers::UpdatePriceTierRequest>,
) -> ApiResult<Json<crate::tiers::PriceTier>> {
    Ok(Json(
        crate::tiers::update_tier(&state.pool, &user.claims, id, req).await?,
    ))
}

async fn get_product_tier_prices(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Vec<crate::tiers::ProductTierPriceRow>>> {
    Ok(Json(
        crate::tiers::get_product_tier_prices(&state.pool, &user.claims, id).await?,
    ))
}

async fn set_product_tier_prices(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(items): Body<Vec<crate::tiers::SetProductTierPriceInput>>,
) -> ApiResult<Json<Vec<crate::tiers::ProductTierPriceRow>>> {
    Ok(Json(
        crate::tiers::set_product_tier_prices(&state.pool, &user.claims, id, items).await?,
    ))
}

#[derive(Deserialize)]
struct PromotionsQuery {
    #[serde(default)]
    status: Option<String>,
}

async fn list_promotions(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<PromotionsQuery>,
) -> ApiResult<Json<Vec<crate::promotions::PromotionDto>>> {
    Ok(Json(
        crate::promotions::list(&state.pool, &user.claims, q.status).await?,
    ))
}

async fn create_promotion(
    State(state): State<AppState>,
    user: AuthUser,
    Body(req): Body<crate::promotions::CreatePromotionInput>,
) -> ApiResult<(StatusCode, Json<crate::promotions::PromotionDto>)> {
    let promo = crate::promotions::create(&state.pool, &user.claims, req).await?;
    Ok((StatusCode::CREATED, Json(promo)))
}

async fn update_promotion(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(req): Body<crate::promotions::CreatePromotionInput>,
) -> ApiResult<Json<crate::promotions::PromotionDto>> {
    Ok(Json(
        crate::promotions::update(&state.pool, &user.claims, id, req).await?,
    ))
}

async fn activate_promotion(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    crate::promotions::activate(&state.pool, &user.claims, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn pause_promotion(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    crate::promotions::pause(&state.pool, &user.claims, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_coupons(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<Vec<crate::coupons::CouponDto>>> {
    Ok(Json(crate::coupons::list(&state.pool, &user.claims).await?))
}

async fn create_coupon(
    State(state): State<AppState>,
    user: AuthUser,
    Body(req): Body<crate::coupons::CreateCouponInput>,
) -> ApiResult<(StatusCode, Json<crate::coupons::CouponDto>)> {
    let coupon = crate::coupons::create(&state.pool, &user.claims, req).await?;
    Ok((StatusCode::CREATED, Json(coupon)))
}

async fn update_coupon(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(req): Body<crate::coupons::CreateCouponInput>,
) -> ApiResult<Json<crate::coupons::CouponDto>> {
    Ok(Json(
        crate::coupons::update(&state.pool, &user.claims, id, req).await?,
    ))
}

async fn toggle_coupon(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<crate::coupons::CouponDto>> {
    Ok(Json(
        crate::coupons::toggle(&state.pool, &user.claims, id).await?,
    ))
}

// ---------------------------------------------------------- devoluciones (F1.11)

#[derive(Deserialize)]
struct RefundListQuery {
    branch_id: Uuid,
    #[serde(default)]
    status: Option<String>,
}

async fn create_refund(
    State(state): State<AppState>,
    user: AuthUser,
    Body(req): Body<refunds::RefundRequest>,
) -> ApiResult<(StatusCode, Json<refunds::Refund>)> {
    match refunds::create(&state.pool, &user.claims, req).await? {
        refunds::Created::New(r) => Ok((StatusCode::CREATED, Json(r))),
        refunds::Created::Replayed(r) => Ok((StatusCode::OK, Json(r))),
    }
}

async fn approve_refund(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<refunds::Refund>> {
    Ok(Json(refunds::approve(&state.pool, &user.claims, id).await?))
}

async fn reject_refund(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(req): Body<refunds::RejectRequest>,
) -> ApiResult<Json<refunds::Refund>> {
    Ok(Json(
        refunds::reject(&state.pool, &user.claims, id, req).await?,
    ))
}

async fn list_refunds(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<RefundListQuery>,
) -> ApiResult<Json<Vec<refunds::Refund>>> {
    Ok(Json(
        refunds::list(&state.pool, &user.claims, q.branch_id, q.status).await?,
    ))
}

async fn order_refunds(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<Vec<refunds::Refund>>> {
    Ok(Json(
        refunds::for_order(&state.pool, &user.claims, id).await?,
    ))
}

// ------------------------------------------------------------ anulación (F1.12)

async fn void_sale(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(req): Body<sales::VoidRequest>,
) -> ApiResult<Json<orders::Order>> {
    Ok(Json(sales::void(&state.pool, &user.claims, id, req).await?))
}

// ------------------------------------------------------- historial (F1.13)

async fn sales_history_list(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<sales_history::HistoryQuery>,
) -> ApiResult<Json<sales_history::HistoryPage>> {
    Ok(Json(
        sales_history::list(&state.pool, &user.claims, q).await?,
    ))
}

async fn sales_history_detail(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<sales_history::SaleDetail>> {
    Ok(Json(
        sales_history::detail(&state.pool, &user.claims, id).await?,
    ))
}

async fn business_info(
    State(state): State<AppState>,
    user: AuthUser,
) -> ApiResult<Json<settings::BusinessInfo>> {
    Ok(Json(
        settings::business_info(&state.pool, &user.claims).await?,
    ))
}

// ------------------------------------------------------- importacion CSV (F2.6)

async fn import_analyze(
    State(state): State<AppState>,
    user: AuthUser,
    headers: axum::http::HeaderMap,
    body: axum::body::Bytes,
) -> ApiResult<Json<crate::csv_import::AnalyzeResponse>> {
    let content_type = headers
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let (branch_id, csv_bytes) = if content_type.contains("multipart/form-data") {
        let boundary = content_type
            .split(';')
            .find_map(|p| {
                let p = p.trim();
                p.strip_prefix("boundary=")
            })
            .ok_or_else(|| {
                error::coded(
                    StatusCode::BAD_REQUEST,
                    "MISSING_BOUNDARY",
                    "Falta el delimitador (boundary) en Content-Type",
                    None,
                )
            })?;

        let parts = crate::csv_import::parse_multipart_body(&body, boundary.trim_matches('"'));
        let branch_id_bytes = parts
            .get("branch_id")
            .map(|(_, d)| d.as_slice())
            .ok_or_else(|| {
                error::coded(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "MISSING_FIELD",
                    "Falta el campo branch_id",
                    None,
                )
            })?;
        let branch_id_str = String::from_utf8_lossy(branch_id_bytes).trim().to_string();
        let branch_id = branch_id_str.parse::<Uuid>().map_err(|_| {
            error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "INVALID_UUID",
                "branch_id inválido",
                None,
            )
        })?;

        let file_bytes = parts.get("file").map(|(_, d)| d.clone()).ok_or_else(|| {
            error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "MISSING_FIELD",
                "Falta el archivo CSV (campo 'file')",
                None,
            )
        })?;

        (branch_id, file_bytes)
    } else {
        #[derive(Deserialize)]
        struct JsonImportReq {
            branch_id: Uuid,
            #[serde(default)]
            file_content: Option<String>,
            #[serde(default)]
            csv: Option<String>,
        }
        let json_body: JsonImportReq = serde_json::from_slice(&body).map_err(|e| {
            error::coded(
                StatusCode::BAD_REQUEST,
                "INVALID_BODY",
                format!("Cuerpo inválido: {e}"),
                None,
            )
        })?;
        let content = json_body.file_content.or(json_body.csv).unwrap_or_default();
        (json_body.branch_id, content.into_bytes())
    };

    let res =
        crate::csv_import::analyze_csv(&state.pool, &user.claims, branch_id, &csv_bytes).await?;
    Ok(Json(res))
}

async fn import_commit(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<crate::csv_import::CommitRequest>,
) -> ApiResult<Json<crate::csv_import::CommitResponse>> {
    let res = crate::csv_import::commit_import(&state.pool, &user.claims, req).await?;
    Ok(Json(res))
}
