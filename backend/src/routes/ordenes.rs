//! Endpoints de órdenes de mesa, domicilio y mayorista.

use super::*;

pub(super) fn router() -> Router<AppState> {
    Router::new()
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
}

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
