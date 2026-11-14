//! Endpoints de clientes y cuentas por cobrar (fiado).

use super::*;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        // Clientes (F1.4, F3.1)
        .route("/v1/customers", get(list_customers).post(create_customer))
        .route("/v1/customers/upsert", post(upsert_customer))
        .route("/v1/customers/{id}", get(get_customer).put(update_customer))
        .route("/v1/customers/{id}/archive", post(archive_customer))
        .route("/v1/customers/{id}/unarchive", post(unarchive_customer))
        .route("/v1/customers/{id}/purchases", get(list_customer_purchases))
        .route("/v1/customers/{id}/statement", get(get_customer_statement))
        // Cuentas por cobrar (F3.3, F3.5)
        .route("/v1/receivables", get(list_receivables))
        .route("/v1/receivables/summary", get(get_receivables_summary))
        .route("/v1/receivables/{id}", get(get_receivable))
        .route(
            "/v1/receivables/{id}/payments",
            post(create_receivable_payment),
        )
        .route(
            "/v1/receivables/{id}/whatsapp-link",
            get(get_receivable_whatsapp_link),
        )
}

async fn list_receivables(
    State(state): State<AppState>,
    user: AuthUser,
    Query(filter): Query<receivables::ReceivablesFilter>,
) -> ApiResult<Json<Vec<receivables::ReceivableRow>>> {
    let (scope, mut tx) = tenancy::begin(&state.pool, &user.claims).await?;
    let rows = receivables::list_receivables_tx(&mut tx, &scope, filter).await?;
    tx.commit().await?;
    Ok(Json(rows))
}

async fn get_receivable(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<receivables::ReceivableDetail>> {
    let (scope, mut tx) = tenancy::begin(&state.pool, &user.claims).await?;
    let detail = receivables::get_receivable_detail_tx(&mut tx, &scope, id).await?;
    tx.commit().await?;
    Ok(Json(detail))
}

async fn create_receivable_payment(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(body): Body<receivables::CreateReceivablePaymentRequest>,
) -> ApiResult<(StatusCode, Json<receivables::ReceivablePaymentResponse>)> {
    let (status, resp) = receivables::record_payment(&state.pool, &user.claims, id, body).await?;
    Ok((status, Json(resp)))
}

async fn get_receivables_summary(
    State(state): State<AppState>,
    user: AuthUser,
    Query(query): Query<receivables::SummaryQuery>,
) -> ApiResult<Json<receivables::ReceivablesSummaryResponse>> {
    let summary = receivables::get_summary(&state.pool, &user.claims, query.branch_id).await?;
    Ok(Json(summary))
}

async fn get_customer_statement(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Query(query): Query<receivables::StatementQuery>,
) -> ApiResult<Json<receivables::CustomerStatementResponse>> {
    let stmt =
        receivables::get_customer_statement(&state.pool, &user.claims, id, query.from, query.to)
            .await?;
    Ok(Json(stmt))
}

async fn get_receivable_whatsapp_link(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<receivables::WhatsappLinkResponse>> {
    let link = receivables::get_whatsapp_link(&state.pool, &user.claims, id).await?;
    Ok(Json(link))
}

async fn list_customers(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<customers::CustomerQuery>,
) -> ApiResult<Json<Vec<customers::CustomerListItem>>> {
    Ok(Json(customers::list(&state.pool, &user.claims, q).await?))
}

async fn get_customer(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<customers::CustomerDetail>> {
    Ok(Json(customers::get(&state.pool, &user.claims, id).await?))
}

async fn create_customer(
    State(state): State<AppState>,
    user: AuthUser,
    Body(input): Body<customers::CreateCustomerInput>,
) -> ApiResult<(StatusCode, Json<customers::CustomerDetail>)> {
    let customer = customers::create(&state.pool, &user.claims, input).await?;
    Ok((StatusCode::CREATED, Json(customer)))
}

async fn update_customer(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Body(input): Body<customers::CreateCustomerInput>,
) -> ApiResult<Json<customers::CustomerDetail>> {
    Ok(Json(
        customers::update(&state.pool, &user.claims, id, input).await?,
    ))
}

async fn upsert_customer(
    State(state): State<AppState>,
    user: AuthUser,
    Body(input): Body<customers::CreateCustomerInput>,
) -> ApiResult<Json<customers::CustomerUpsertResponse>> {
    Ok(Json(
        customers::upsert(&state.pool, &user.claims, input).await?,
    ))
}

async fn archive_customer(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    customers::archive(&state.pool, &user.claims, id).await?;
    Ok(StatusCode::OK)
}

async fn unarchive_customer(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<StatusCode> {
    customers::unarchive(&state.pool, &user.claims, id).await?;
    Ok(StatusCode::OK)
}

async fn list_customer_purchases(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
    Query(q): Query<customers::CustomerPurchasesQuery>,
) -> ApiResult<Json<crate::sales_history::HistoryPage>> {
    Ok(Json(
        customers::list_purchases(&state.pool, &user.claims, id, q.page, q.page_size).await?,
    ))
}
