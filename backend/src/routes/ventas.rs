//! Endpoints de venta de mostrador, caja, devoluciones, anulaciones e historial.

use super::*;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        // Venta de mostrador y caja
        .route("/v1/sales", get(sales_history_list).post(create_sale))
        .route("/v1/sales/{id}", get(sales_history_detail))
        .route("/v1/sales/quote", post(sales_quote))
        .route("/v1/sales/{id}/void", post(void_sale))
        .route("/v1/credit/schedule-preview", get(schedule_preview))
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
}

/// 201 si la venta es nueva; 200 si es un reintento de una ya registrada.
async fn create_sale(
    State(state): State<AppState>,
    user: AuthUser,
    Body(req): Body<SaleRequest>,
) -> ApiResult<(StatusCode, Json<sales::SaleResponse>)> {
    Ok(
        match sales::checkout(&state.pool, &user.claims, req).await? {
            Checkout::Created(r) => {
                // El documento POS sale ya, para que el tiquete lleve CUDE y QR.
                crate::fiscal::kick(&state.pool);
                (StatusCode::CREATED, Json(r))
            }
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

async fn schedule_preview(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<crate::credit::SchedulePreviewQuery>,
) -> ApiResult<Json<crate::credit::SchedulePreviewResponse>> {
    Ok(Json(
        crate::credit::preview(&state.pool, &user.claims, q).await?,
    ))
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

async fn void_sale(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(req): Body<sales::VoidRequest>,
) -> ApiResult<Json<orders::Order>> {
    Ok(Json(sales::void(&state.pool, &user.claims, id, req).await?))
}

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
