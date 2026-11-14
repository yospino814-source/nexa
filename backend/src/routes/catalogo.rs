//! Endpoints de catálogo: productos, modificadores, tarifas, promociones, cupones e importación CSV.

use super::*;

pub(super) fn router() -> Router<AppState> {
    Router::new()
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
}

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
