//! Endpoints de inventario, kardex, traslados y bodegas.

use super::*;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        // Inventario
        .route("/v1/inventory/available", get(available))
        .route("/v1/inventory/movements", get(movements))
        .route("/v1/inventory/replenish", post(replenish))
        .route("/v1/inventory/adjust", post(adjust))
        .route("/v1/inventory/adjustments", get(list_adjustments))
        .route(
            "/v1/inventory/adjustments/{id}/approve",
            post(approve_adjustment),
        )
        .route(
            "/v1/inventory/adjustments/{id}/reject",
            post(reject_adjustment),
        )
        .route("/v1/inventory/count", post(count))
        .route("/v1/inventory/transfer", post(transfer))
        .route("/v1/inventory/stock", get(stock_levels))
        .route("/v1/inventory/alerts", get(inventory_alerts))
        .route("/v1/inventory/expiring", get(inventory_expiring))
        .route("/v1/inventory/min-qty", put(set_min_qty))
        // Bodegas / Ubicaciones (F2.8)
        .route("/v1/locations", get(list_locations).post(create_location))
        .route("/v1/locations/{id}", put(update_location))
}

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
struct ExpiringQuery {
    branch_id: Option<Uuid>,
    days: Option<i64>,
}

async fn inventory_expiring(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<ExpiringQuery>,
) -> ApiResult<Json<Vec<inventory::ExpiringLot>>> {
    let lots =
        inventory::list_expiring_lots(&state.pool, &user.claims, q.branch_id, q.days).await?;
    Ok(Json(lots))
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
    #[serde(default)]
    lot_id: Option<Uuid>,
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
            lot_id: req.lot_id,
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
    Body(req): Body<inventory::CreateAdjustmentRequest>,
) -> ApiResult<Json<inventory::InventoryAdjustment>> {
    let (scope, mut tx) =
        tenancy::begin_in_branch(&state.pool, &user.claims, req.branch_id).await?;
    let adj = inventory::create_adjustment_tx(&mut tx, &scope, req).await?;
    tx.commit().await?;
    Ok(Json(adj))
}

#[derive(Deserialize, Default)]
struct AdjustmentsQuery {
    branch_id: Option<Uuid>,
    status: Option<String>,
}

async fn list_adjustments(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<AdjustmentsQuery>,
) -> ApiResult<Json<Vec<inventory::InventoryAdjustment>>> {
    let adjs =
        inventory::list_adjustments(&state.pool, &user.claims, q.branch_id, q.status).await?;
    Ok(Json(adjs))
}

async fn approve_adjustment(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<inventory::InventoryAdjustment>> {
    let (scope, mut tx) = tenancy::begin(&state.pool, &user.claims).await?;
    let adj = inventory::approve_adjustment_tx(&mut tx, &scope, id).await?;
    tx.commit().await?;
    Ok(Json(adj))
}

async fn reject_adjustment(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(req): Body<inventory::RejectAdjustmentRequest>,
) -> ApiResult<Json<inventory::InventoryAdjustment>> {
    let (scope, mut tx) = tenancy::begin(&state.pool, &user.claims).await?;
    let adj = inventory::reject_adjustment_tx(&mut tx, &scope, id, &req.reason).await?;
    tx.commit().await?;
    Ok(Json(adj))
}
