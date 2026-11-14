//! Endpoints de proveedores, compras, cuentas por pagar y gastos.

use super::*;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        // Proveedores
        .route("/v1/suppliers", get(list_suppliers).post(create_supplier))
        .route("/v1/suppliers/{id}", get(get_supplier).put(update_supplier))
        .route("/v1/suppliers/{id}/archive", post(archive_supplier))
        .route("/v1/suppliers/{id}/products", put(set_supplier_products))
        // Compras
        .route("/v1/purchases", get(list_purchases).post(create_purchase))
        .route("/v1/purchases/summary", get(purchases_summary))
        .route("/v1/purchases/cost-preview", post(cost_preview_purchase))
        .route("/v1/purchases/suggestions", get(get_purchase_suggestions))
        .route(
            "/v1/purchases/suggestions/create",
            post(create_purchases_from_suggestions),
        )
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
}

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

async fn get_purchase_suggestions(
    State(state): State<AppState>,
    user: AuthUser,
    Query(query): Query<procurement::SuggestionsQuery>,
) -> ApiResult<Json<Vec<procurement::PurchaseSuggestionGroup>>> {
    let branch_id = query
        .branch_id
        .or_else(|| user.claims.branch_id())
        .ok_or_else(|| crate::error::validation("branch_id is required"))?;
    let suggestions =
        procurement::get_purchase_suggestions(&state.pool, &user.claims, branch_id).await?;
    Ok(Json(suggestions))
}

async fn create_purchases_from_suggestions(
    State(state): State<AppState>,
    user: AuthUser,
    Json(req): Json<procurement::CreateFromSuggestionsRequest>,
) -> ApiResult<(StatusCode, Json<Vec<Uuid>>)> {
    let ids =
        procurement::create_purchases_from_suggestions(&state.pool, &user.claims, req).await?;
    Ok((StatusCode::CREATED, Json(ids)))
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
