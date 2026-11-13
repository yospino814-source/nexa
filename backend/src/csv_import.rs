#![allow(clippy::collapsible_if, clippy::needless_range_loop)]

use std::collections::{BTreeMap, HashMap, HashSet};

use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{self, ApiError};
use crate::tenancy::{self, Action, Scope};

type Tx<'a> = sqlx::Transaction<'a, sqlx::Postgres>;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRowData {
    pub sku: Option<String>,
    pub codigo_barras: Option<String>,
    pub nombre: String,
    pub categoria: Option<String>,
    pub unidad: String,
    pub impuesto: String,
    pub costo: Option<Decimal>,
    pub precio: Option<Decimal>,
    pub cantidad_inicial: Option<Decimal>,
    pub sku_padre: Option<String>,
    pub atributos: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportRowResult {
    pub line: usize,
    pub status: String, // "OK" | "ADVERTENCIA" | "ERROR"
    pub action: String, // "CREAR" | "ACTUALIZAR"
    pub messages: Vec<String>,
    pub data: ImportRowData,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImportSummary {
    pub ok: usize,
    pub warnings: usize,
    pub errors: usize,
    pub create: usize,
    pub update: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeResponse {
    pub session_id: Uuid,
    pub summary: ImportSummary,
    pub rows: Vec<ImportRowResult>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CommitRequest {
    pub session_id: Uuid,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommitResponse {
    pub committed_rows: usize,
    pub done: bool,
}

/// Parsea un archivo CSV en filas considerando delimitador ';' o ',' y comillas dobles.
pub fn parse_csv_records(content: &str) -> Result<Vec<Vec<String>>, ApiError> {
    let clean = content.strip_prefix('\u{feff}').unwrap_or(content);
    let mut lines = clean.lines().filter(|l| !l.trim().is_empty());
    let first_line = match lines.next() {
        Some(l) => l,
        None => {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CSV_EMPTY",
                "El archivo CSV está vacío",
                None,
            ));
        }
    };

    let comma_count = first_line.matches(',').count();
    let semi_count = first_line.matches(';').count();
    let delimiter = if semi_count > comma_count { ';' } else { ',' };

    let mut records = Vec::new();
    let mut current_field = String::new();
    let mut current_record = Vec::new();
    let mut in_quotes = false;
    let mut chars = clean.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '"' => {
                if in_quotes && chars.peek() == Some(&'"') {
                    // Doble comilla escapada
                    chars.next();
                    current_field.push('"');
                } else {
                    in_quotes = !in_quotes;
                }
            }
            c if c == delimiter && !in_quotes => {
                current_record.push(current_field.trim().to_string());
                current_field.clear();
            }
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                if !in_quotes {
                    current_record.push(current_field.trim().to_string());
                    current_field.clear();
                    if !current_record.iter().all(|f| f.is_empty()) {
                        records.push(std::mem::take(&mut current_record));
                    }
                } else {
                    current_field.push('\n');
                }
            }
            '\n' => {
                if !in_quotes {
                    current_record.push(current_field.trim().to_string());
                    current_field.clear();
                    if !current_record.iter().all(|f| f.is_empty()) {
                        records.push(std::mem::take(&mut current_record));
                    }
                } else {
                    current_field.push('\n');
                }
            }
            other => {
                current_field.push(other);
            }
        }
    }

    if !current_field.is_empty() || !current_record.is_empty() {
        current_record.push(current_field.trim().to_string());
        if !current_record.iter().all(|f| f.is_empty()) {
            records.push(current_record);
        }
    }

    Ok(records)
}

/// Extrae partes de un payload multipart/form-data.
pub fn parse_multipart_body(
    body: &[u8],
    boundary: &str,
) -> HashMap<String, (Option<String>, Vec<u8>)> {
    let mut parts = HashMap::new();
    let delimiter = format!("--{boundary}");
    let delimiter_bytes = delimiter.as_bytes();

    let mut start = 0;
    while let Some(pos) = find_subsequence(&body[start..], delimiter_bytes) {
        let part_start = start + pos + delimiter_bytes.len();
        if part_start >= body.len() || &body[part_start..part_start.min(body.len())] == b"--" {
            break;
        }

        // Buscar fin de cabeceras (\r\n\r\n)
        let header_end_marker = b"\r\n\r\n";
        if let Some(h_end) = find_subsequence(&body[part_start..], header_end_marker) {
            let headers_bytes = &body[part_start..part_start + h_end];
            let content_start = part_start + h_end + header_end_marker.len();

            // Buscar siguiente delimitador
            let content_end =
                if let Some(next_del) = find_subsequence(&body[content_start..], delimiter_bytes) {
                    // Quitar el \r\n anterior al delimitador si existe
                    let mut end = content_start + next_del;
                    if end >= 2 && &body[end - 2..end] == b"\r\n" {
                        end -= 2;
                    }
                    end
                } else {
                    body.len()
                };

            let headers_str = String::from_utf8_lossy(headers_bytes);
            let mut name = None;
            let mut filename = None;

            for line in headers_str.lines() {
                if line.to_lowercase().starts_with("content-disposition:") {
                    for param in line.split(';') {
                        let param = param.trim();
                        if let Some(stripped) = param.strip_prefix("name=") {
                            name = Some(stripped.trim_matches('"').to_string());
                        } else if let Some(stripped) = param.strip_prefix("filename=") {
                            filename = Some(stripped.trim_matches('"').to_string());
                        }
                    }
                }
            }

            if let Some(n) = name {
                let data = body[content_start..content_end].to_vec();
                parts.insert(n, (filename, data));
            }

            start = content_end;
        } else {
            break;
        }
    }

    parts
}

fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack.windows(needle.len()).position(|w| w == needle)
}

/// Analiza un archivo CSV y genera la sesión de importación.
pub async fn analyze_csv(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Uuid,
    csv_bytes: &[u8],
) -> Result<AnalyzeResponse, ApiError> {
    if csv_bytes.len() > 2 * 1024 * 1024 {
        return Err(error::coded(
            StatusCode::PAYLOAD_TOO_LARGE,
            "FILE_TOO_LARGE",
            "El archivo excede el límite máximo de 2 MB",
            None,
        ));
    }

    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, branch_id).await?;
    scope.require(Action::ManageInventory)?;

    let csv_text = String::from_utf8(csv_bytes.to_vec()).map_err(|_| {
        error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "CSV_INVALID_UTF8",
            "El archivo no tiene una codificación UTF-8 válida",
            None,
        )
    })?;

    let records = parse_csv_records(&csv_text)?;
    if records.len() < 2 {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "CSV_NO_DATA",
            "El archivo debe tener al menos una fila de datos además de los encabezados",
            None,
        ));
    }

    if records.len() > 5001 {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "CSV_TOO_MANY_ROWS",
            "El archivo excede el límite de 5.000 filas",
            None,
        ));
    }

    // Encabezados
    let headers: Vec<String> = records[0].iter().map(|h| h.trim().to_lowercase()).collect();
    let col_idx = |name: &str| headers.iter().position(|h| h == name);

    let idx_sku = col_idx("sku");
    let idx_barcode = col_idx("codigo_barras");
    let idx_name = col_idx("nombre").ok_or_else(|| {
        error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "CSV_MISSING_HEADER",
            "Falta el encabezado obligatorio 'nombre'",
            None,
        )
    })?;
    let idx_cat = col_idx("categoria");
    let idx_uom = col_idx("unidad");
    let idx_tax = col_idx("impuesto");
    let idx_cost = col_idx("costo");
    let idx_price = col_idx("precio");
    let idx_init_qty = col_idx("cantidad_inicial");
    let idx_parent_sku = col_idx("sku_padre");
    let idx_attrs = col_idx("atributos");

    // Consultar unidades UOM válidas en tbl_tipos_maestra
    let valid_uoms: HashSet<String> = sqlx::query_scalar(
        "select tima_codigo from tbl_tipos_maestra where padre_id = (select tima_id from tbl_tipos_maestra where padre_id is null and tima_codigo = 'UOM')",
    )
    .fetch_all(&mut *tx)
    .await?
    .into_iter()
    .collect();

    // Consultar SKUs existentes en el tenant y si tienen stock
    let existing_skus: HashMap<String, (Uuid, Decimal)> = sqlx::query_as::<_, (String, Uuid, Decimal)>(
        "select p.sku, p.id, coalesce((select sum(s.on_hand) from stock s where s.product_id = p.id and s.branch_id = $2), 0) as on_hand
           from products p
          where p.tenant_id = $1 and p.sku is not null",
    )
    .bind(scope.tenant)
    .bind(branch_id)
    .fetch_all(&mut *tx)
    .await?
    .into_iter()
    .map(|(sku, id, on_hand)| (sku, (id, on_hand)))
    .collect();

    // Consultar categorías existentes en la sucursal
    let existing_categories: HashSet<String> =
        sqlx::query_scalar("select lower(name) from product_categories where branch_id = $1")
            .bind(branch_id)
            .fetch_all(&mut *tx)
            .await?
            .into_iter()
            .collect();

    // SKUs definidos en el propio archivo para verificar referencias de variantes
    let file_skus: HashSet<String> = records[1..]
        .iter()
        .filter_map(|r| idx_sku.and_then(|i| r.get(i)).map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty())
        .collect();

    let valid_taxes: HashSet<&str> = ["IVA19", "IVA5", "INC8", "EXENTO"].into_iter().collect();

    let mut row_results = Vec::new();
    let mut summary = ImportSummary::default();

    for (row_num, row) in records[1..].iter().enumerate() {
        let line = row_num + 2; // Fila 1 es encabezado, fila 2 es primer dato
        let get_val = |opt_idx: Option<usize>| -> Option<String> {
            opt_idx
                .and_then(|i| row.get(i))
                .map(|s| s.trim().to_string())
        };

        let raw_name = get_val(Some(idx_name)).unwrap_or_default();
        let raw_sku = get_val(idx_sku).filter(|s| !s.is_empty());
        let raw_barcode = get_val(idx_barcode).filter(|s| !s.is_empty());
        let raw_cat = get_val(idx_cat).filter(|s| !s.is_empty());
        let raw_uom = get_val(idx_uom).unwrap_or_else(|| "UND".to_string());
        let raw_tax = get_val(idx_tax).unwrap_or_else(|| "IVA19".to_string());
        let raw_cost = get_val(idx_cost).filter(|s| !s.is_empty());
        let raw_price = get_val(idx_price).filter(|s| !s.is_empty());
        let raw_init_qty = get_val(idx_init_qty).filter(|s| !s.is_empty());
        let raw_parent_sku = get_val(idx_parent_sku).filter(|s| !s.is_empty());
        let raw_attrs_str = get_val(idx_attrs).unwrap_or_default();

        let mut attrs = BTreeMap::new();
        if !raw_attrs_str.is_empty() {
            for pair in raw_attrs_str.split('|') {
                if let Some((k, v)) = pair.split_once('=') {
                    if !k.trim().is_empty() && !v.trim().is_empty() {
                        attrs.insert(k.trim().to_string(), v.trim().to_string());
                    }
                }
            }
        }

        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // 1. nombre vacío
        if raw_name.is_empty() {
            errors.push("nombre vacío".to_string());
        }

        // 2. codigo_barras en notación científica
        if let Some(ref bc) = raw_barcode {
            if bc.contains("E+") || bc.contains("e+") {
                errors.push("código en notación científica".to_string());
            }
        }

        // 3. impuesto fuera de IVA19, IVA5, INC8, EXENTO
        let tax_code = raw_tax.to_uppercase();
        if !valid_taxes.contains(tax_code.as_str()) {
            errors.push(format!("impuesto inválido: {tax_code}"));
        }

        // 4. unidad no existe en UOM
        let uom_code = raw_uom.to_uppercase();
        if !valid_uoms.contains(&uom_code) {
            errors.push(format!("unidad no existe: {uom_code}"));
        }

        // 5. precio o costo no numérico o negativo
        let parsed_cost = if let Some(ref c_str) = raw_cost {
            match c_str.parse::<Decimal>() {
                Ok(val) if val >= Decimal::ZERO => Some(val),
                _ => {
                    errors.push("costo no numérico o negativo".to_string());
                    None
                }
            }
        } else {
            None
        };

        let parsed_price = if let Some(ref p_str) = raw_price {
            match p_str.parse::<Decimal>() {
                Ok(val) if val >= Decimal::ZERO => Some(val),
                _ => {
                    errors.push("precio no numérico o negativo".to_string());
                    None
                }
            }
        } else {
            None
        };

        let parsed_init_qty = if let Some(ref q_str) = raw_init_qty {
            match q_str.parse::<Decimal>() {
                Ok(val) if val >= Decimal::ZERO => Some(val),
                _ => {
                    errors.push("cantidad_inicial no numérica o negativa".to_string());
                    None
                }
            }
        } else {
            None
        };

        // 6. sku existe en el negocio -> OK con acción ACTUALIZAR
        // 7. sku no existe -> acción CREAR
        let action = if let Some(ref s) = raw_sku {
            if existing_skus.contains_key(s) {
                "ACTUALIZAR"
            } else {
                "CREAR"
            }
        } else {
            "CREAR"
        };

        // 8. categoria no existe en la sucursal -> ADVERTENCIA "se creará la categoría"
        if let Some(ref c) = raw_cat {
            if !existing_categories.contains(&c.to_lowercase()) {
                warnings.push("se creará la categoría".to_string());
            }
        }

        // 9. sku_padre con atributos -> la fila es una variante; si no existe ni está en el archivo: ERROR
        if let Some(ref parent_sku) = raw_parent_sku {
            if !attrs.is_empty()
                && !existing_skus.contains_key(parent_sku)
                && !file_skus.contains(parent_sku)
            {
                errors.push("padre no existe".to_string());
            }
        }

        // 10. cantidad_inicial > 0 y el producto ya tiene stock -> ADVERTENCIA "se ignora la cantidad inicial"
        if let Some(init_q) = parsed_init_qty {
            if init_q > Decimal::ZERO {
                if let Some(ref s) = raw_sku {
                    if let Some((_, on_hand)) = existing_skus.get(s) {
                        if *on_hand > Decimal::ZERO {
                            warnings.push("se ignora la cantidad inicial".to_string());
                        }
                    }
                }
            }
        }

        let status = if !errors.is_empty() {
            summary.errors += 1;
            "ERROR"
        } else if !warnings.is_empty() {
            summary.warnings += 1;
            "ADVERTENCIA"
        } else {
            summary.ok += 1;
            "OK"
        };

        if action == "CREAR" {
            summary.create += 1;
        } else {
            summary.update += 1;
        }

        let mut messages = errors;
        messages.extend(warnings);

        row_results.push(ImportRowResult {
            line,
            status: status.to_string(),
            action: action.to_string(),
            messages,
            data: ImportRowData {
                sku: raw_sku,
                codigo_barras: raw_barcode,
                nombre: raw_name,
                categoria: raw_cat,
                unidad: uom_code,
                impuesto: tax_code,
                costo: parsed_cost,
                precio: parsed_price,
                cantidad_inicial: parsed_init_qty,
                sku_padre: raw_parent_sku,
                atributos: attrs,
            },
        });
    }

    // Guardar sesión en import_sessions
    let rows_json = serde_json::to_value(&row_results)
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Error serializando filas: {e}")))?;

    let session_id: Uuid = sqlx::query_scalar(
        "insert into import_sessions (tenant_id, branch_id, created_by, rows, committed_rows)
         values ($1, $2, $3, $4, 0)
         returning id",
    )
    .bind(scope.tenant)
    .bind(branch_id)
    .bind(scope.user)
    .bind(rows_json)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(AnalyzeResponse {
        session_id,
        summary,
        rows: row_results,
    })
}

/// Aplica un bloque de filas analizadas previamente.
pub async fn commit_import(
    pool: &PgPool,
    claims: &Claims,
    req: CommitRequest,
) -> Result<CommitResponse, ApiError> {
    if req.limit > 200 {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "LIMIT_EXCEEDED",
            "El límite máximo por lote es 200",
            None,
        ));
    }

    // 1. Iniciar transacción en el tenant
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require(Action::ManageInventory)?;

    // 2. Cargar sesión
    let session_row: Option<(Uuid, serde_json::Value, i32, DateTime<Utc>)> = sqlx::query_as(
        "select branch_id, rows, committed_rows, expires_at
           from import_sessions
          where tenant_id = $1 and id = $2
            for update",
    )
    .bind(scope.tenant)
    .bind(req.session_id)
    .fetch_optional(&mut *tx)
    .await?;

    let (branch_id, rows_json, current_committed, expires_at) = session_row.ok_or_else(|| {
        error::coded(
            StatusCode::NOT_FOUND,
            "SESSION_NOT_FOUND",
            "Sesión de importación no encontrada",
            None,
        )
    })?;

    if expires_at < Utc::now() {
        return Err(error::coded(
            StatusCode::GONE,
            "IMPORT_EXPIRED",
            "La sesión de importación ha expirado (más de 24 horas)",
            None,
        ));
    }

    let rows: Vec<ImportRowResult> = serde_json::from_value(rows_json)
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("Error deserializando filas: {e}")))?;

    // Con al menos un ERROR, responde 409 IMPORT_HAS_ERRORS
    if rows.iter().any(|r| r.status == "ERROR") {
        return Err(error::coded(
            StatusCode::CONFLICT,
            "IMPORT_HAS_ERRORS",
            "No se puede confirmar una importación con errores",
            None,
        ));
    }

    let total_rows = rows.len();
    let current_committed_usize = current_committed as usize;

    // Rango de filas a aplicar
    let start_idx = req.offset;
    let end_idx = (req.offset + req.limit).min(total_rows);

    for idx in start_idx..end_idx {
        // Idempotencia: Si la fila ya fue comprometida previamente, se ignora
        if idx < current_committed_usize {
            continue;
        }

        let item = &rows[idx];
        let data = &item.data;

        // Resolver o crear categoría si está especificada
        let category_id: Option<Uuid> = if let Some(ref cat_name) = data.categoria {
            let cat_id: Option<Uuid> = sqlx::query_scalar(
                "select id from product_categories where branch_id = $1 and lower(name) = lower($2)",
            )
            .bind(branch_id)
            .bind(cat_name)
            .fetch_optional(&mut *tx)
            .await?;

            if let Some(id) = cat_id {
                Some(id)
            } else {
                let new_cat_id: Uuid = sqlx::query_scalar(
                    "insert into product_categories (tenant_id, branch_id, name)
                     values ($1, $2, $3)
                     returning id",
                )
                .bind(scope.tenant)
                .bind(branch_id)
                .bind(cat_name)
                .fetch_one(&mut *tx)
                .await?;
                Some(new_cat_id)
            }
        } else {
            None
        };

        // Buscar si es variante con padre
        let is_variant = data.sku_padre.is_some() && !data.atributos.is_empty();

        if is_variant {
            let parent_sku = data.sku_padre.as_ref().unwrap();
            let parent_id_opt: Option<Uuid> =
                sqlx::query_scalar("select id from products where tenant_id = $1 and sku = $2")
                    .bind(scope.tenant)
                    .bind(parent_sku)
                    .fetch_optional(&mut *tx)
                    .await?;

            if let Some(parent_id) = parent_id_opt {
                sqlx::query(
                    "update products set has_variants = true where tenant_id = $1 and id = $2",
                )
                .bind(scope.tenant)
                .bind(parent_id)
                .execute(&mut *tx)
                .await?;

                // Crear variante
                let var_input = crate::products::CreateVariantInput {
                    sku: data.sku.clone(),
                    barcode: data.codigo_barras.clone(),
                    attrs: data.atributos.clone(),
                    price: data.precio.unwrap_or_default(),
                    cost_price: data.costo,
                };
                let variant_id =
                    crate::products::create_variant_tx(&mut tx, &scope, parent_id, var_input)
                        .await?;

                // Asignar categoría en la sucursal si existe
                if let Some(cid) = category_id {
                    sqlx::query(
                        "update branch_products set category_id = $1 where branch_id = $2 and product_id = $3",
                    )
                    .bind(cid)
                    .bind(branch_id)
                    .bind(variant_id)
                    .execute(&mut *tx)
                    .await?;
                }

                // Carga de inventario inicial si corresponde
                if let Some(init_q) = data.cantidad_inicial {
                    if init_q > Decimal::ZERO {
                        apply_initial_stock(
                            &mut tx,
                            &scope,
                            branch_id,
                            variant_id,
                            init_q,
                            data.costo,
                            req.session_id,
                        )
                        .await?;
                    }
                }
            }
        } else {
            // Producto regular (padre o independiente)
            let existing_id: Option<Uuid> = if let Some(ref s) = data.sku {
                sqlx::query_scalar("select id from products where tenant_id = $1 and sku = $2")
                    .bind(scope.tenant)
                    .bind(s)
                    .fetch_optional(&mut *tx)
                    .await?
            } else {
                None
            };

            let product_id = if let Some(id) = existing_id {
                // Actualizar
                let p_input = crate::products::ProductInput {
                    name: data.nombre.clone(),
                    sku: data.sku.clone(),
                    barcode: data.codigo_barras.clone(),
                    kind: "FISICO".into(),
                    usage: "VENTA".into(),
                    unit_measure_code: data.unidad.clone(),
                    purchase_uom_code: None,
                    purchase_factor: Decimal::ONE,
                    tax_code: data.impuesto.clone(),
                    cost_price: data.costo,
                    base_price: data.precio,
                    reorder_qty: None,
                    max_qty: None,
                    track_lots: false,
                    allow_price_override: true,
                    has_variants: false,
                    description: None,
                };
                crate::products::update_product_tx(&mut tx, &scope, id, p_input).await?;
                id
            } else {
                let is_parent = data
                    .sku
                    .as_ref()
                    .map(|s| {
                        rows.iter()
                            .any(|r| r.data.sku_padre.as_deref() == Some(s.as_str()))
                    })
                    .unwrap_or(false);

                // Crear
                let p_input = crate::products::ProductInput {
                    name: data.nombre.clone(),
                    sku: data.sku.clone(),
                    barcode: data.codigo_barras.clone(),
                    kind: "FISICO".into(),
                    usage: "VENTA".into(),
                    unit_measure_code: data.unidad.clone(),
                    purchase_uom_code: None,
                    purchase_factor: Decimal::ONE,
                    tax_code: data.impuesto.clone(),
                    cost_price: data.costo,
                    base_price: data.precio,
                    reorder_qty: None,
                    max_qty: None,
                    track_lots: false,
                    allow_price_override: true,
                    has_variants: is_parent,
                    description: None,
                };
                crate::products::create_product_tx(&mut tx, &scope, p_input).await?
            };

            // Categoría en la sucursal
            if let Some(cid) = category_id {
                sqlx::query(
                    "update branch_products set category_id = $1 where branch_id = $2 and product_id = $3",
                )
                .bind(cid)
                .bind(branch_id)
                .bind(product_id)
                .execute(&mut *tx)
                .await?;
            }

            // Cantidad inicial
            if let Some(init_q) = data.cantidad_inicial {
                if init_q > Decimal::ZERO {
                    let current_on_hand: Decimal = sqlx::query_scalar(
                        "select coalesce(sum(on_hand), 0) from stock where branch_id = $1 and product_id = $2",
                    )
                    .bind(branch_id)
                    .bind(product_id)
                    .fetch_one(&mut *tx)
                    .await?;

                    if current_on_hand == Decimal::ZERO {
                        apply_initial_stock(
                            &mut tx,
                            &scope,
                            branch_id,
                            product_id,
                            init_q,
                            data.costo,
                            req.session_id,
                        )
                        .await?;
                    }
                }
            }
        }
    }

    let new_committed = current_committed_usize.max(end_idx);
    sqlx::query(
        "update import_sessions
            set committed_rows = $1
          where tenant_id = $2 and id = $3",
    )
    .bind(new_committed as i32)
    .bind(scope.tenant)
    .bind(req.session_id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    Ok(CommitResponse {
        committed_rows: new_committed,
        done: new_committed >= total_rows,
    })
}

async fn apply_initial_stock(
    tx: &mut Tx<'_>,
    scope: &Scope,
    branch_id: Uuid,
    product_id: Uuid,
    qty: Decimal,
    cost: Option<Decimal>,
    session_id: Uuid,
) -> Result<(), ApiError> {
    let loc_opt: Option<Uuid> =
        sqlx::query_scalar("select id from locations where branch_id = $1 and is_default = true")
            .bind(branch_id)
            .fetch_optional(&mut **tx)
            .await?;

    let loc_id = match loc_opt {
        Some(id) => id,
        None => {
            return Err(error::coded(
                StatusCode::CONFLICT,
                "IMPORT_NO_DEFAULT_LOCATION",
                "La sucursal no tiene una bodega principal",
                None,
            ));
        }
    };

    crate::inventory::apply_delta_tx(
        tx,
        scope,
        crate::inventory::StockDelta {
            branch_id,
            location_id: loc_id,
            product_id,
            delta: qty,
            movement: "AJUSTE_ENTRADA",
            unit_cost: cost,
            lot_id: None,
            ref_type: "IMPORT",
            ref_id: session_id,
            cause: Some("Carga inicial".to_string()),
        },
    )
    .await?;

    if let Some(c) = cost {
        sqlx::query(
            "update products set cost_price = coalesce(cost_price, $1) where tenant_id = $2 and id = $3",
        )
        .bind(c)
        .bind(scope.tenant)
        .bind(product_id)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}
