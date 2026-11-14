//! Endpoints de reportes y tablero.

use super::*;

pub(super) fn router() -> Router<AppState> {
    Router::new()
        // Reportes
        .route("/v1/reports/dashboard", get(get_dashboard))
        .route("/v1/reports/sales-by-day", get(report_sales_by_day))
        .route("/v1/reports/sales-by-product", get(report_sales_by_product))
        .route(
            "/v1/reports/sales-by-category",
            get(report_sales_by_category),
        )
        .route("/v1/reports/sales-by-cashier", get(report_sales_by_cashier))
        .route("/v1/reports/sales-by-method", get(report_sales_by_method))
        .route("/v1/reports/sales-heatmap", get(report_sales_heatmap))
        .route(
            "/v1/reports/inventory-valuation",
            get(report_inventory_valuation),
        )
        .route(
            "/v1/reports/inventory-rotation",
            get(report_inventory_rotation),
        )
        .route(
            "/v1/reports/purchases-by-supplier",
            get(report_purchases_by_supplier),
        )
}

async fn get_dashboard(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<reports::dashboard::DashboardQuery>,
) -> ApiResult<Json<reports::dashboard::DashboardResponse>> {
    Ok(Json(
        reports::dashboard::get(&state.pool, &user.claims, q).await?,
    ))
}

async fn report_sales_by_day(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<reports::queries::ReportQuery>,
) -> ApiResult<Response> {
    Ok(reports::queries::sales_by_day(&state.pool, &user.claims, q)
        .await?
        .into_response())
}

async fn report_sales_by_product(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<reports::queries::ReportQuery>,
) -> ApiResult<Response> {
    Ok(
        reports::queries::sales_by_product(&state.pool, &user.claims, q)
            .await?
            .into_response(),
    )
}

async fn report_sales_by_category(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<reports::queries::ReportQuery>,
) -> ApiResult<Response> {
    Ok(
        reports::queries::sales_by_category(&state.pool, &user.claims, q)
            .await?
            .into_response(),
    )
}

async fn report_sales_by_cashier(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<reports::queries::ReportQuery>,
) -> ApiResult<Response> {
    Ok(
        reports::queries::sales_by_cashier(&state.pool, &user.claims, q)
            .await?
            .into_response(),
    )
}

async fn report_sales_by_method(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<reports::queries::ReportQuery>,
) -> ApiResult<Response> {
    Ok(
        reports::queries::sales_by_method(&state.pool, &user.claims, q)
            .await?
            .into_response(),
    )
}

async fn report_sales_heatmap(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<reports::queries::ReportQuery>,
) -> ApiResult<Json<reports::queries::SalesHeatmapResponse>> {
    Ok(Json(
        reports::queries::sales_heatmap(&state.pool, &user.claims, q).await?,
    ))
}

async fn report_inventory_valuation(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<reports::queries::ReportQuery>,
) -> ApiResult<Response> {
    Ok(
        reports::queries::inventory_valuation(&state.pool, &user.claims, q)
            .await?
            .into_response(),
    )
}

async fn report_inventory_rotation(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<reports::queries::ReportQuery>,
) -> ApiResult<Response> {
    Ok(
        reports::queries::inventory_rotation(&state.pool, &user.claims, q)
            .await?
            .into_response(),
    )
}

async fn report_purchases_by_supplier(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<reports::queries::ReportQuery>,
) -> ApiResult<Response> {
    Ok(
        reports::queries::purchases_by_supplier(&state.pool, &user.claims, q)
            .await?
            .into_response(),
    )
}
