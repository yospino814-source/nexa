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
use crate::error::{ApiError, DomainError};
use crate::orders::OrderStatus;
use crate::sales::{Checkout, SaleRequest};
use crate::{
    AppState, audit, cash, catalog, inventory, orders, permissions, procurement, registers, sales,
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
        // Venta de mostrador y caja
        .route("/v1/sales", post(create_sale))
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
        // Inventario
        .route("/v1/inventory/available", get(available))
        .route("/v1/inventory/movements", get(movements))
        .route("/v1/inventory/replenish", post(replenish))
        .route("/v1/inventory/adjust", post(adjust))
        .route("/v1/inventory/count", post(count))
        .route("/v1/inventory/transfer", post(transfer))
        .route("/v1/inventory/stock", get(stock_levels))
        .route("/v1/inventory/min-qty", put(set_min_qty))
        // Proveedores
        .route("/v1/suppliers", get(list_suppliers).post(create_supplier))
        .route("/v1/suppliers/{id}", put(update_supplier))
        // Compras
        .route("/v1/purchases", get(list_purchases).post(create_purchase))
        .route("/v1/purchases/summary", get(purchases_summary))
        .route("/v1/purchases/{id}", get(get_purchase))
        .route("/v1/purchases/{id}/receive", post(receive_purchase))
        .route("/v1/purchases/{id}/cancel", post(cancel_purchase))
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
            q.limit.unwrap_or(200),
        )
        .await?,
    ))
}

/// Existencias de la sucursal con estado frente al punto de reorden.
async fn stock_levels(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<BranchQuery>,
) -> ApiResult<Json<Vec<inventory::StockLevel>>> {
    Ok(Json(
        inventory::stock_levels(&state.pool, &user.claims, q.branch_id).await?,
    ))
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
    from_branch_id: Uuid,
    to_branch_id: Uuid,
    product_id: Uuid,
    qty: Decimal,
    cause: String,
}

/// Traslado de unidades entre sucursales del mismo negocio.
async fn transfer(
    State(state): State<AppState>,
    user: AuthUser,
    Body(req): Body<TransferBody>,
) -> ApiResult<(StatusCode, Json<serde_json::Value>)> {
    let reference = inventory::transfer(
        &state.pool,
        &user.claims,
        req.from_branch_id,
        req.to_branch_id,
        req.product_id,
        req.qty,
        &req.cause,
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

async fn receive_purchase(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<procurement::PurchaseDetail>> {
    let detail = procurement::receive_purchase(&state.pool, &user.claims, id).await?;
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
