#![allow(clippy::collapsible_if)]
//! Directorio de clientes completos para POS, facturación y cartera (F1.4, F3.1).

use axum::http::StatusCode;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{self, ApiError, DomainError};
use crate::tenancy::{self, Scope};

type Tx<'a> = Transaction<'a, Postgres>;

const VALID_DOC_TYPES: &[&str] = &["CC", "NIT", "CE", "PP", "TI", "PEP", "PPT", "NIUP"];
const ALLOWED_RESPONSIBILITIES: &[&str] = &["O-13", "O-15", "O-23", "O-47", "R-99-PN"];

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Customer {
    pub id: Uuid,
    pub doc_type: Option<String>,
    pub doc_number: Option<String>,
    pub dv: Option<String>,
    pub name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub price_tier_id: Option<Uuid>,
    pub person_type: String,
    pub tax_regime: String,
    pub fiscal_responsibilities: Vec<String>,
    pub ciiu: Option<String>,
    pub withholding_agent: bool,
    pub city_code: Option<String>,
    pub billing_address: Option<String>,
    pub shipping_address: Option<String>,
    pub pinned_note: Option<String>,
    pub credit_limit: Decimal,
    pub credit_block_days: i32,
    pub archived_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct CustomerListItem {
    pub id: Uuid,
    pub doc_type: Option<String>,
    pub doc_number: Option<String>,
    pub dv: Option<String>,
    pub name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub price_tier_id: Option<Uuid>,
    pub person_type: String,
    pub tax_regime: String,
    pub fiscal_responsibilities: Vec<String>,
    pub ciiu: Option<String>,
    pub withholding_agent: bool,
    pub city_code: Option<String>,
    pub billing_address: Option<String>,
    pub shipping_address: Option<String>,
    pub pinned_note: Option<String>,
    pub credit_limit: Decimal,
    pub credit_block_days: i32,
    pub archived_at: Option<DateTime<Utc>>,
    pub balance: Decimal,
    pub max_days_overdue: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CustomerDetail {
    #[serde(flatten)]
    pub customer: Customer,
    pub balance: Decimal,
    pub credit_available: Decimal,
    pub wallet_balance: Decimal,
    pub missing_fiscal: Vec<String>,
}

impl std::ops::Deref for CustomerDetail {
    type Target = Customer;
    fn deref(&self) -> &Self::Target {
        &self.customer
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct CustomerUpsertResponse {
    pub customer: CustomerDetail,
    pub result: &'static str,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct CreateCustomerInput {
    #[serde(default)]
    pub doc_type: Option<String>,
    #[serde(default)]
    pub doc_number: Option<String>,
    #[serde(default)]
    pub dv: Option<String>,
    pub name: String,
    #[serde(default)]
    pub phone: Option<String>,
    #[serde(default)]
    pub email: Option<String>,
    #[serde(default)]
    pub price_tier_id: Option<Uuid>,
    #[serde(default)]
    pub tax_regime: Option<String>,
    #[serde(default)]
    pub fiscal_responsibilities: Option<Vec<String>>,
    #[serde(default)]
    pub ciiu: Option<String>,
    #[serde(default)]
    pub withholding_agent: Option<bool>,
    #[serde(default)]
    pub city_code: Option<String>,
    #[serde(default)]
    pub billing_address: Option<String>,
    #[serde(default)]
    pub shipping_address: Option<String>,
    #[serde(default)]
    pub pinned_note: Option<String>,
    #[serde(default)]
    pub credit_limit: Option<Decimal>,
    #[serde(default)]
    pub credit_block_days: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct CustomerQuery {
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub page: Option<i64>,
    #[serde(default)]
    pub page_size: Option<i64>,
    #[serde(default)]
    pub limit: Option<i64>,
}

#[derive(Debug, Deserialize)]
pub struct CustomerPurchasesQuery {
    #[serde(default)]
    pub page: Option<i64>,
    #[serde(default)]
    pub page_size: Option<i64>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct NormalizedCustomer {
    pub doc_type: Option<String>,
    pub doc_number: Option<String>,
    pub dv: Option<String>,
    pub name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
    pub price_tier_id: Option<Uuid>,
    pub person_type: String,
    pub tax_regime: String,
    pub fiscal_responsibilities: Vec<String>,
    pub ciiu: Option<String>,
    pub withholding_agent: bool,
    pub city_code: Option<String>,
    pub billing_address: Option<String>,
    pub shipping_address: Option<String>,
    pub pinned_note: Option<String>,
    pub credit_limit: Decimal,
    pub credit_block_days: i32,
}

/// Calcula la lista de campos faltantes para facturación electrónica en orden estricto:
/// 1. doc_type y doc_number -> "documento"
/// 2. email -> "correo"
/// 3. city_code -> "municipio"
/// 4. billing_address -> "dirección de facturación"
pub fn compute_missing_fiscal(
    doc_type: Option<&str>,
    doc_number: Option<&str>,
    email: Option<&str>,
    city_code: Option<&str>,
    billing_address: Option<&str>,
) -> Vec<String> {
    let mut missing = Vec::new();
    let has_doc_type = doc_type.map(str::trim).filter(|s| !s.is_empty()).is_some();
    let has_doc_number = doc_number
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .is_some();
    if !has_doc_type || !has_doc_number {
        missing.push("documento".to_string());
    }
    if email.map(str::trim).filter(|s| !s.is_empty()).is_none() {
        missing.push("correo".to_string());
    }
    if city_code.map(str::trim).filter(|s| !s.is_empty()).is_none() {
        missing.push("municipio".to_string());
    }
    if billing_address
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .is_none()
    {
        missing.push("dirección de facturación".to_string());
    }
    missing
}

/// Normalización pura antes de guardar o buscar (F3.1).
///
/// 1. doc_type: mayúsculas y sin espacios a los lados.
/// 2. Si doc_number contiene -: lo que va después del último guion es el DV y lo de antes es el número.
///    Si además llegó dv y es distinto: 422 CUSTOMER_DV_CONFLICT.
/// 3. doc_number: solo letras y dígitos, en mayúsculas.
/// 4. Si doc_type = NIT:
///    - el DV se calcula con nit_dv (fase 2); si llegó uno distinto: 422 NIT_DV_INVALID;
///      si no llegó, se guarda el calculado;
///    - person_type = JURIDICA.
/// 5. Con cualquier otro tipo: dv = null y person_type = NATURAL. person_type nunca se acepta del cliente.
pub fn normalize_customer(input: CreateCustomerInput) -> Result<NormalizedCustomer, ApiError> {
    // 1. Name: espacios internos reducidos a uno, trim
    let name = input.name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() || name.len() > 120 {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "CUSTOMER_INVALID",
            "El nombre debe tener entre 1 y 120 caracteres",
            None,
        ));
    }

    // 1. doc_type: mayúsculas y sin espacios a los lados
    let doc_type = input.doc_type.and_then(|raw| {
        let clean = raw.trim().to_ascii_uppercase();
        if clean.is_empty() { None } else { Some(clean) }
    });

    if let Some(ref dt) = doc_type {
        if !VALID_DOC_TYPES.contains(&dt.as_str()) {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CUSTOMER_INVALID",
                "Tipo de documento no válido",
                None,
            ));
        }
    }

    // 2. Si doc_number contiene '-': extraer parte número y parte DV
    let mut candidate_dv: Option<String> = input.dv.and_then(|raw| {
        let clean: String = raw.trim().chars().filter(|c| c.is_ascii_digit()).collect();
        if clean.is_empty() { None } else { Some(clean) }
    });

    let raw_doc_number = input.doc_number.and_then(|raw| {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    });

    let unparsed_number = if let Some(ref raw_num) = raw_doc_number {
        if let Some(idx) = raw_num.rfind('-') {
            let num_part = &raw_num[..idx];
            let dv_part = &raw_num[idx + 1..];
            let clean_dv: String = dv_part
                .trim()
                .chars()
                .filter(|c| c.is_ascii_digit())
                .collect();
            if !clean_dv.is_empty() {
                if let Some(ref explicit_dv) = candidate_dv {
                    if explicit_dv != &clean_dv {
                        return Err(error::coded(
                            StatusCode::UNPROCESSABLE_ENTITY,
                            "CUSTOMER_DV_CONFLICT",
                            "El dígito de verificación no coincide.",
                            None,
                        ));
                    }
                }
                candidate_dv = Some(clean_dv);
            }
            Some(num_part.to_string())
        } else {
            Some(raw_num.clone())
        }
    } else {
        None
    };

    // 3. doc_number: solo letras y dígitos, en mayúsculas
    let doc_number = unparsed_number.and_then(|raw| {
        let clean: String = raw
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_ascii_uppercase())
            .collect();
        if clean.is_empty() { None } else { Some(clean) }
    });

    // Si viene doc_number debe haber doc_type
    if doc_number.is_some() && doc_type.is_none() {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "CUSTOMER_INVALID",
            "El número de documento requiere un tipo de documento",
            None,
        ));
    }

    // 4. Si doc_type = NIT: calcular DV con nit_dv y person_type = JURIDICA
    // 5. Con cualquier otro tipo: dv = null y person_type = NATURAL
    let (dv, person_type) = if doc_type.as_deref() == Some("NIT") {
        let nit_clean = doc_number.as_deref().unwrap_or("");
        if nit_clean.is_empty() {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CUSTOMER_INVALID",
                "El NIT requiere un número de documento",
                None,
            ));
        }
        let expected_dv = crate::procurement::nit_dv(nit_clean).to_string();
        if let Some(ref cand) = candidate_dv {
            if cand != &expected_dv {
                return Err(error::coded(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "NIT_DV_INVALID",
                    "El dígito de verificación no coincide con el NIT.",
                    None,
                ));
            }
        }
        (Some(expected_dv), "JURIDICA".to_string())
    } else {
        (None, "NATURAL".to_string())
    };

    // Phone: solo dígitos, 7..15
    let phone = input.phone.and_then(|raw| {
        let clean: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
        if clean.is_empty() { None } else { Some(clean) }
    });

    if let Some(ref p) = phone {
        if p.len() < 7 || p.len() > 15 {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CUSTOMER_INVALID",
                "El teléfono debe tener entre 7 y 15 dígitos",
                None,
            ));
        }
    }

    // Email: minúsculas, trim, exactamente una '@'
    let email = input.email.and_then(|raw| {
        let clean = raw.trim().to_ascii_lowercase();
        if clean.is_empty() { None } else { Some(clean) }
    });

    if let Some(ref e) = email {
        let at_count = e.chars().filter(|c| *c == '@').count();
        if at_count != 1 || e.starts_with('@') || e.ends_with('@') || e.len() < 3 {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CUSTOMER_INVALID",
                "El correo electrónico no es válido",
                None,
            ));
        }
    }

    // Tax regime
    let tax_regime = input
        .tax_regime
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "NO_RESPONSABLE".to_string());
    if tax_regime != "RESPONSABLE_IVA" && tax_regime != "NO_RESPONSABLE" {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "CUSTOMER_INVALID",
            "Régimen tributario no válido",
            None,
        ));
    }

    // Responsabilidades fiscales
    let fiscal_responsibilities = input
        .fiscal_responsibilities
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| vec!["R-99-PN".to_string()]);
    for code in &fiscal_responsibilities {
        if !ALLOWED_RESPONSIBILITIES.contains(&code.as_str()) {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CUSTOMER_INVALID",
                "Código de responsabilidad fiscal no válido",
                None,
            ));
        }
    }

    // CIIU (4 dígitos numéricos)
    let ciiu = input.ciiu.and_then(|raw| {
        let clean = raw.trim().to_string();
        if clean.is_empty() { None } else { Some(clean) }
    });
    if let Some(ref c) = ciiu {
        if c.len() != 4 || !c.chars().all(|ch| ch.is_ascii_digit()) {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CUSTOMER_INVALID",
                "El código CIIU debe ser de 4 dígitos numéricos",
                None,
            ));
        }
    }

    let withholding_agent = input.withholding_agent.unwrap_or(false);

    // City code (5 dígitos numéricos)
    let city_code = input.city_code.and_then(|raw| {
        let clean = raw.trim().to_string();
        if clean.is_empty() { None } else { Some(clean) }
    });
    if let Some(ref c) = city_code {
        if c.len() != 5 || !c.chars().all(|ch| ch.is_ascii_digit()) {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CUSTOMER_INVALID",
                "El código de municipio debe ser de 5 dígitos numéricos",
                None,
            ));
        }
    }

    // Direcciones y notas
    let billing_address = input.billing_address.and_then(|raw| {
        let clean = raw.trim().to_string();
        if clean.is_empty() { None } else { Some(clean) }
    });
    if let Some(ref a) = billing_address {
        if a.len() > 200 {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CUSTOMER_INVALID",
                "La dirección de facturación no puede superar 200 caracteres",
                None,
            ));
        }
    }

    let shipping_address = input.shipping_address.and_then(|raw| {
        let clean = raw.trim().to_string();
        if clean.is_empty() { None } else { Some(clean) }
    });
    if let Some(ref a) = shipping_address {
        if a.len() > 200 {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CUSTOMER_INVALID",
                "La dirección de despacho no puede superar 200 caracteres",
                None,
            ));
        }
    }

    let pinned_note = input.pinned_note.and_then(|raw| {
        let clean = raw.trim().to_string();
        if clean.is_empty() { None } else { Some(clean) }
    });
    if let Some(ref n) = pinned_note {
        if n.len() > 300 {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CUSTOMER_INVALID",
                "La nota fijada no puede superar 300 caracteres",
                None,
            ));
        }
    }

    let credit_limit = input.credit_limit.unwrap_or(Decimal::ZERO);
    if credit_limit < Decimal::ZERO {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "CUSTOMER_INVALID",
            "El cupo de crédito no puede ser negativo",
            None,
        ));
    }

    let credit_block_days = input.credit_block_days.unwrap_or(30);
    if !(0..=365).contains(&credit_block_days) {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "CUSTOMER_INVALID",
            "Los días de bloqueo de crédito deben estar entre 0 y 365",
            None,
        ));
    }

    Ok(NormalizedCustomer {
        doc_type,
        doc_number,
        dv,
        name,
        phone,
        email,
        price_tier_id: input.price_tier_id,
        person_type,
        tax_regime,
        fiscal_responsibilities,
        ciiu,
        withholding_agent,
        city_code,
        billing_address,
        shipping_address,
        pinned_note,
        credit_limit,
        credit_block_days,
    })
}

/// Lista clientes con balance y días de mora (F3.1).
pub async fn list(
    pool: &PgPool,
    claims: &Claims,
    query: CustomerQuery,
) -> Result<Vec<CustomerListItem>, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("customers:read")?;

    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.or(query.limit).unwrap_or(50).clamp(1, 100);
    let offset = (page - 1) * page_size;
    let q = query.q.unwrap_or_default().trim().to_string();
    let status = query
        .status
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "TODOS");

    let rows = sqlx::query_as::<_, CustomerListItem>(
        "with cust_receivables as (
            select
                r.customer_id,
                coalesce(sum(r.amount_total - r.amount_paid), 0) as balance,
                coalesce(max(
                    case
                        when inst.earliest_due is not null then
                            greatest(0, (current_date - inst.earliest_due)::int)
                        else
                            greatest(0, (current_date - r.due_date)::int)
                    end
                ), 0) as max_days_overdue
            from receivables r
            left join (
                select receivable_id, min(due_date) as earliest_due
                from receivable_installments
                where amount_paid < amount
                group by receivable_id
            ) inst on inst.receivable_id = r.id
            where r.tenant_id = $1 and r.status in ('ABIERTA', 'PARCIAL')
            group by r.customer_id
        )
        select
            c.id, c.doc_type, c.doc_number, c.dv, c.name, c.phone, c.email, c.price_tier_id,
            c.person_type, c.tax_regime, c.fiscal_responsibilities, c.ciiu, c.withholding_agent,
            c.city_code, c.billing_address, c.shipping_address, c.pinned_note, c.credit_limit,
            c.credit_block_days, c.archived_at,
            coalesce(cr.balance, 0) as balance,
            case when coalesce(cr.balance, 0) > 0 then coalesce(cr.max_days_overdue, 0) else 0 end as max_days_overdue
        from customers c
        left join cust_receivables cr on cr.customer_id = c.id
        where c.tenant_id = $1
          and (
            $2::text is null
            or ($2 = 'ARCHIVADO' and c.archived_at is not null)
            or ($2 = 'AL_DIA' and c.archived_at is null and (coalesce(cr.balance, 0) = 0 or coalesce(cr.max_days_overdue, 0) = 0))
            or ($2 = 'EN_MORA' and c.archived_at is null and coalesce(cr.balance, 0) > 0 and coalesce(cr.max_days_overdue, 0) > 0)
          )
          and (
            case when $2::text = 'ARCHIVADO' then true else c.archived_at is null end
          )
          and (
            $3::text = ''
            or c.name ilike '%' || $3 || '%'
            or c.doc_number ilike $3 || '%'
            or c.phone ilike $3 || '%'
          )
        order by c.name asc
        limit $4 offset $5",
    )
    .bind(scope.tenant)
    .bind(status)
    .bind(&q)
    .bind(page_size)
    .bind(offset)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(rows)
}

/// Alias para búsqueda retrocompatible.
pub async fn search(
    pool: &PgPool,
    claims: &Claims,
    query: CustomerQuery,
) -> Result<Vec<CustomerListItem>, ApiError> {
    list(pool, claims, query).await
}

/// Obtiene el detalle de un cliente con saldos y missing_fiscal.
pub async fn get(
    pool: &PgPool,
    claims: &Claims,
    customer_id: Uuid,
) -> Result<CustomerDetail, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("customers:read")?;

    let detail = get_customer_detail_tx(&mut tx, &scope, customer_id).await?;
    tx.commit().await?;
    Ok(detail)
}

pub async fn get_customer_detail_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    customer_id: Uuid,
) -> Result<CustomerDetail, ApiError> {
    let customer = sqlx::query_as::<_, Customer>(
        "select id, doc_type, doc_number, dv, name, phone, email, price_tier_id,
                person_type, tax_regime, fiscal_responsibilities, ciiu, withholding_agent,
                city_code, billing_address, shipping_address, pinned_note, credit_limit,
                credit_block_days, archived_at
           from customers
          where tenant_id = $1 and id = $2",
    )
    .bind(scope.tenant)
    .bind(customer_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(ApiError::Domain(DomainError::NotFound))?;

    let balance: Decimal = sqlx::query_scalar(
        "select coalesce(sum(amount_total - amount_paid), 0)
           from receivables
          where tenant_id = $1 and customer_id = $2 and status in ('ABIERTA', 'PARCIAL')",
    )
    .bind(scope.tenant)
    .bind(customer_id)
    .fetch_one(&mut **tx)
    .await?;

    let wallet_balance: Decimal = sqlx::query_scalar(
        "select coalesce(balance, 0) from customer_wallets where tenant_id = $1 and customer_id = $2",
    )
    .bind(scope.tenant)
    .bind(customer_id)
    .fetch_optional(&mut **tx)
    .await?
    .unwrap_or(Decimal::ZERO);

    let credit_available = (customer.credit_limit - balance).max(Decimal::ZERO);

    let missing_fiscal = compute_missing_fiscal(
        customer.doc_type.as_deref(),
        customer.doc_number.as_deref(),
        customer.email.as_deref(),
        customer.city_code.as_deref(),
        customer.billing_address.as_deref(),
    );

    Ok(CustomerDetail {
        customer,
        balance,
        credit_available,
        wallet_balance,
        missing_fiscal,
    })
}

/// Crea un cliente nuevo en el negocio.
pub async fn create(
    pool: &PgPool,
    claims: &Claims,
    input: CreateCustomerInput,
) -> Result<CustomerDetail, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("customers:write")?;

    let customer = create_tx(&mut tx, &scope, input).await?;
    let detail = get_customer_detail_tx(&mut tx, &scope, customer.id).await?;
    tx.commit().await?;
    Ok(detail)
}

pub async fn create_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    input: CreateCustomerInput,
) -> Result<Customer, ApiError> {
    let norm = normalize_customer(input)?;

    // Verificar si ya existe un cliente con el mismo documento activo
    if let (Some(dt), Some(dn)) = (&norm.doc_type, &norm.doc_number) {
        let existing: Option<Uuid> = sqlx::query_scalar(
            "select id from customers where tenant_id = $1 and doc_type = $2 and doc_number = $3 and archived_at is null",
        )
        .bind(scope.tenant)
        .bind(dt)
        .bind(dn)
        .fetch_optional(&mut **tx)
        .await?;

        if let Some(existing_id) = existing {
            return Err(error::coded(
                StatusCode::CONFLICT,
                "CUSTOMER_DUPLICATED",
                "Ya existe un cliente registrado con este documento",
                Some(serde_json::json!({ "id": existing_id })),
            ));
        }
    }

    let customer = sqlx::query_as::<_, Customer>(
        "insert into customers (
            tenant_id, doc_type, doc_number, dv, name, phone, email, price_tier_id,
            person_type, tax_regime, fiscal_responsibilities, ciiu, withholding_agent,
            city_code, billing_address, shipping_address, pinned_note, credit_limit,
            credit_block_days
         )
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)
         returning id, doc_type, doc_number, dv, name, phone, email, price_tier_id,
                   person_type, tax_regime, fiscal_responsibilities, ciiu, withholding_agent,
                   city_code, billing_address, shipping_address, pinned_note, credit_limit,
                   credit_block_days, archived_at",
    )
    .bind(scope.tenant)
    .bind(&norm.doc_type)
    .bind(&norm.doc_number)
    .bind(&norm.dv)
    .bind(&norm.name)
    .bind(&norm.phone)
    .bind(&norm.email)
    .bind(norm.price_tier_id)
    .bind(&norm.person_type)
    .bind(&norm.tax_regime)
    .bind(&norm.fiscal_responsibilities)
    .bind(&norm.ciiu)
    .bind(norm.withholding_agent)
    .bind(&norm.city_code)
    .bind(&norm.billing_address)
    .bind(&norm.shipping_address)
    .bind(&norm.pinned_note)
    .bind(norm.credit_limit)
    .bind(norm.credit_block_days)
    .fetch_one(&mut **tx)
    .await?;

    Ok(customer)
}

/// Actualiza un cliente existente en el negocio.
pub async fn update(
    pool: &PgPool,
    claims: &Claims,
    customer_id: Uuid,
    input: CreateCustomerInput,
) -> Result<CustomerDetail, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("customers:write")?;

    let existing = sqlx::query_as::<_, Customer>(
        "select id, doc_type, doc_number, dv, name, phone, email, price_tier_id,
                person_type, tax_regime, fiscal_responsibilities, ciiu, withholding_agent,
                city_code, billing_address, shipping_address, pinned_note, credit_limit,
                credit_block_days, archived_at
           from customers
          where tenant_id = $1 and id = $2",
    )
    .bind(scope.tenant)
    .bind(customer_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError::Domain(DomainError::NotFound))?;

    let norm = normalize_customer(input)?;

    // credit_limit y credit_block_days solo los cambia quien tiene credit:override;
    // si no lo tiene y cambian: 403.
    let credit_limit_changed = norm.credit_limit != existing.credit_limit;
    let credit_block_days_changed = norm.credit_block_days != existing.credit_block_days;
    if (credit_limit_changed || credit_block_days_changed)
        && !scope.has_permission("credit:override")
    {
        return Err(ApiError::Domain(DomainError::Forbidden));
    }

    // Verificar unicidad de documento si cambió
    if let (Some(dt), Some(dn)) = (&norm.doc_type, &norm.doc_number) {
        let conflict: Option<Uuid> = sqlx::query_scalar(
            "select id from customers
              where tenant_id = $1 and doc_type = $2 and doc_number = $3 and id <> $4 and archived_at is null",
        )
        .bind(scope.tenant)
        .bind(dt)
        .bind(dn)
        .bind(customer_id)
        .fetch_optional(&mut *tx)
        .await?;

        if let Some(conf_id) = conflict {
            return Err(error::coded(
                StatusCode::CONFLICT,
                "CUSTOMER_DUPLICATED",
                "Ya existe otro cliente registrado con este documento",
                Some(serde_json::json!({ "id": conf_id })),
            ));
        }
    }

    sqlx::query(
        "update customers
            set doc_type = $1, doc_number = $2, dv = $3, name = $4, phone = $5,
                email = $6, price_tier_id = $7, person_type = $8, tax_regime = $9,
                fiscal_responsibilities = $10, ciiu = $11, withholding_agent = $12,
                city_code = $13, billing_address = $14, shipping_address = $15,
                pinned_note = $16, credit_limit = $17, credit_block_days = $18
          where tenant_id = $19 and id = $20",
    )
    .bind(&norm.doc_type)
    .bind(&norm.doc_number)
    .bind(&norm.dv)
    .bind(&norm.name)
    .bind(&norm.phone)
    .bind(&norm.email)
    .bind(norm.price_tier_id)
    .bind(&norm.person_type)
    .bind(&norm.tax_regime)
    .bind(&norm.fiscal_responsibilities)
    .bind(&norm.ciiu)
    .bind(norm.withholding_agent)
    .bind(&norm.city_code)
    .bind(&norm.billing_address)
    .bind(&norm.shipping_address)
    .bind(&norm.pinned_note)
    .bind(norm.credit_limit)
    .bind(norm.credit_block_days)
    .bind(scope.tenant)
    .bind(customer_id)
    .execute(&mut *tx)
    .await?;

    let detail = get_customer_detail_tx(&mut tx, &scope, customer_id).await?;
    tx.commit().await?;
    Ok(detail)
}

/// Buscar o crear (upsert) para POS y formularios (F3.1).
///
/// 1. Normaliza.
/// 2. Busca, sin archivados, primero por email (si viene) y luego por doc_type y doc_number (si vienen).
/// 3. Si lo encuentra: copia solo los campos del cuerpo que en la base estén en null o vacíos.
///    Nunca sobrescribe un valor que ya existe. Respuesta: {customer, result: "UPDATED"}, o "UNCHANGED" si no cambió nada.
/// 4. Si no lo encuentra: lo crea. Respuesta: {customer, result: "CREATED"}.
pub async fn upsert(
    pool: &PgPool,
    claims: &Claims,
    input: CreateCustomerInput,
) -> Result<CustomerUpsertResponse, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("customers:write")?;

    let norm = normalize_customer(input)?;

    // 2. Buscar primero por email (si viene)
    let mut existing: Option<Customer> = None;
    if let Some(ref em) = norm.email {
        existing = sqlx::query_as::<_, Customer>(
            "select id, doc_type, doc_number, dv, name, phone, email, price_tier_id,
                    person_type, tax_regime, fiscal_responsibilities, ciiu, withholding_agent,
                    city_code, billing_address, shipping_address, pinned_note, credit_limit,
                    credit_block_days, archived_at
               from customers
              where tenant_id = $1 and archived_at is null and lower(email) = lower($2)
              limit 1",
        )
        .bind(scope.tenant)
        .bind(em)
        .fetch_optional(&mut *tx)
        .await?;
    }

    // Luego por doc_type y doc_number si no se encontró por email
    if existing.is_none() {
        if let (Some(dt), Some(dn)) = (&norm.doc_type, &norm.doc_number) {
            existing = sqlx::query_as::<_, Customer>(
                "select id, doc_type, doc_number, dv, name, phone, email, price_tier_id,
                        person_type, tax_regime, fiscal_responsibilities, ciiu, withholding_agent,
                        city_code, billing_address, shipping_address, pinned_note, credit_limit,
                        credit_block_days, archived_at
                   from customers
                  where tenant_id = $1 and archived_at is null and doc_type = $2 and doc_number = $3
                  limit 1",
            )
            .bind(scope.tenant)
            .bind(dt)
            .bind(dn)
            .fetch_optional(&mut *tx)
            .await?;
        }
    }

    match existing {
        Some(ex) => {
            let mut changed = false;
            let mut up_doc_type = ex.doc_type.clone();
            let mut up_doc_number = ex.doc_number.clone();
            let mut up_dv = ex.dv.clone();
            let mut up_phone = ex.phone.clone();
            let mut up_email = ex.email.clone();
            let mut up_price_tier_id = ex.price_tier_id;
            let mut up_ciiu = ex.ciiu.clone();
            let mut up_city_code = ex.city_code.clone();
            let mut up_billing_address = ex.billing_address.clone();
            let mut up_shipping_address = ex.shipping_address.clone();
            let mut up_pinned_note = ex.pinned_note.clone();

            if up_doc_type.as_deref().unwrap_or("").trim().is_empty() && norm.doc_type.is_some() {
                up_doc_type = norm.doc_type.clone();
                changed = true;
            }
            if up_doc_number.as_deref().unwrap_or("").trim().is_empty() && norm.doc_number.is_some()
            {
                up_doc_number = norm.doc_number.clone();
                changed = true;
            }
            if up_dv.as_deref().unwrap_or("").trim().is_empty() && norm.dv.is_some() {
                up_dv = norm.dv.clone();
                changed = true;
            }
            if up_phone.as_deref().unwrap_or("").trim().is_empty() && norm.phone.is_some() {
                up_phone = norm.phone.clone();
                changed = true;
            }
            if up_email.as_deref().unwrap_or("").trim().is_empty() && norm.email.is_some() {
                up_email = norm.email.clone();
                changed = true;
            }
            if up_price_tier_id.is_none() && norm.price_tier_id.is_some() {
                up_price_tier_id = norm.price_tier_id;
                changed = true;
            }
            if up_ciiu.as_deref().unwrap_or("").trim().is_empty() && norm.ciiu.is_some() {
                up_ciiu = norm.ciiu.clone();
                changed = true;
            }
            if up_city_code.as_deref().unwrap_or("").trim().is_empty() && norm.city_code.is_some() {
                up_city_code = norm.city_code.clone();
                changed = true;
            }
            if up_billing_address
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
                && norm.billing_address.is_some()
            {
                up_billing_address = norm.billing_address.clone();
                changed = true;
            }
            if up_shipping_address
                .as_deref()
                .unwrap_or("")
                .trim()
                .is_empty()
                && norm.shipping_address.is_some()
            {
                up_shipping_address = norm.shipping_address.clone();
                changed = true;
            }
            if up_pinned_note.as_deref().unwrap_or("").trim().is_empty()
                && norm.pinned_note.is_some()
            {
                up_pinned_note = norm.pinned_note.clone();
                changed = true;
            }

            if changed {
                sqlx::query(
                    "update customers
                        set doc_type = $1, doc_number = $2, dv = $3, phone = $4, email = $5,
                            price_tier_id = $6, ciiu = $7, city_code = $8,
                            billing_address = $9, shipping_address = $10, pinned_note = $11
                      where tenant_id = $12 and id = $13",
                )
                .bind(up_doc_type)
                .bind(up_doc_number)
                .bind(up_dv)
                .bind(up_phone)
                .bind(up_email)
                .bind(up_price_tier_id)
                .bind(up_ciiu)
                .bind(up_city_code)
                .bind(up_billing_address)
                .bind(up_shipping_address)
                .bind(up_pinned_note)
                .bind(scope.tenant)
                .bind(ex.id)
                .execute(&mut *tx)
                .await?;
            }

            let detail = get_customer_detail_tx(&mut tx, &scope, ex.id).await?;
            tx.commit().await?;
            Ok(CustomerUpsertResponse {
                customer: detail,
                result: if changed { "UPDATED" } else { "UNCHANGED" },
            })
        }
        None => {
            let created = sqlx::query_as::<_, Customer>(
                "insert into customers (
                    tenant_id, doc_type, doc_number, dv, name, phone, email, price_tier_id,
                    person_type, tax_regime, fiscal_responsibilities, ciiu, withholding_agent,
                    city_code, billing_address, shipping_address, pinned_note, credit_limit,
                    credit_block_days
                 )
                 values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, $17, $18, $19)
                 returning id, doc_type, doc_number, dv, name, phone, email, price_tier_id,
                           person_type, tax_regime, fiscal_responsibilities, ciiu, withholding_agent,
                           city_code, billing_address, shipping_address, pinned_note, credit_limit,
                           credit_block_days, archived_at",
            )
            .bind(scope.tenant)
            .bind(&norm.doc_type)
            .bind(&norm.doc_number)
            .bind(&norm.dv)
            .bind(&norm.name)
            .bind(&norm.phone)
            .bind(&norm.email)
            .bind(norm.price_tier_id)
            .bind(&norm.person_type)
            .bind(&norm.tax_regime)
            .bind(&norm.fiscal_responsibilities)
            .bind(&norm.ciiu)
            .bind(norm.withholding_agent)
            .bind(&norm.city_code)
            .bind(&norm.billing_address)
            .bind(&norm.shipping_address)
            .bind(&norm.pinned_note)
            .bind(norm.credit_limit)
            .bind(norm.credit_block_days)
            .fetch_one(&mut *tx)
            .await?;

            let detail = get_customer_detail_tx(&mut tx, &scope, created.id).await?;
            tx.commit().await?;
            Ok(CustomerUpsertResponse {
                customer: detail,
                result: "CREATED",
            })
        }
    }
}

/// Archiva un cliente (F3.1).
/// Si tiene saldo en cartera, separados ACTIVO o saldo a favor: 409 CUSTOMER_HAS_BALANCE.
pub async fn archive(pool: &PgPool, claims: &Claims, customer_id: Uuid) -> Result<(), ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("customers:write")?;

    // 1. Saldo en cartera (receivables ABIERTA o PARCIAL)
    let receivables_balance: Decimal = sqlx::query_scalar(
        "select coalesce(sum(amount_total - amount_paid), 0)
           from receivables
          where tenant_id = $1 and customer_id = $2 and status in ('ABIERTA', 'PARCIAL')",
    )
    .bind(scope.tenant)
    .bind(customer_id)
    .fetch_one(&mut *tx)
    .await?;

    if receivables_balance > Decimal::ZERO {
        return Err(error::coded(
            StatusCode::CONFLICT,
            "CUSTOMER_HAS_BALANCE",
            "El cliente tiene saldos pendientes.",
            None,
        ));
    }

    // 2. Separados ACTIVO
    let active_layaways: i64 = sqlx::query_scalar(
        "select count(*) from layaways where tenant_id = $1 and customer_id = $2 and status = 'ACTIVO'",
    )
    .bind(scope.tenant)
    .bind(customer_id)
    .fetch_one(&mut *tx)
    .await?;

    if active_layaways > 0 {
        return Err(error::coded(
            StatusCode::CONFLICT,
            "CUSTOMER_HAS_BALANCE",
            "El cliente tiene saldos pendientes.",
            None,
        ));
    }

    // 3. Saldo a favor (customer_wallets)
    let wallet_balance: Decimal = sqlx::query_scalar(
        "select coalesce(balance, 0) from customer_wallets where tenant_id = $1 and customer_id = $2",
    )
    .bind(scope.tenant)
    .bind(customer_id)
    .fetch_optional(&mut *tx)
    .await?
    .unwrap_or(Decimal::ZERO);

    if wallet_balance > Decimal::ZERO {
        return Err(error::coded(
            StatusCode::CONFLICT,
            "CUSTOMER_HAS_BALANCE",
            "El cliente tiene saldos pendientes.",
            None,
        ));
    }

    let affected = sqlx::query(
        "update customers set archived_at = now() where tenant_id = $1 and id = $2 and archived_at is null",
    )
    .bind(scope.tenant)
    .bind(customer_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    if affected == 0 {
        return Err(ApiError::Domain(DomainError::NotFound));
    }

    tx.commit().await?;
    Ok(())
}

/// Desarchiva un cliente (F3.1).
pub async fn unarchive(pool: &PgPool, claims: &Claims, customer_id: Uuid) -> Result<(), ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("customers:write")?;

    let affected =
        sqlx::query("update customers set archived_at = null where tenant_id = $1 and id = $2")
            .bind(scope.tenant)
            .bind(customer_id)
            .execute(&mut *tx)
            .await?
            .rows_affected();

    if affected == 0 {
        return Err(ApiError::Domain(DomainError::NotFound));
    }

    tx.commit().await?;
    Ok(())
}

/// Historial de ventas del cliente (F3.1).
pub async fn list_purchases(
    pool: &PgPool,
    claims: &Claims,
    customer_id: Uuid,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<crate::sales_history::HistoryPage, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("customers:read")?;

    let page = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(20).clamp(1, 100);
    let offset = (page - 1) * page_size;

    let (count, amount): (i64, Decimal) = sqlx::query_as(
        "select count(*)::bigint, coalesce(sum(o.total_amount), 0)
           from orders o
           join tbl_tipos_maestra st on st.tima_id = o.status_id
          where o.tenant_id = $1 and o.customer_id = $2
            and o.receipt_number is not null
            and st.tima_codigo in ('COBRADA', 'CERRADA', 'CANCELADA')",
    )
    .bind(scope.tenant)
    .bind(customer_id)
    .fetch_one(&mut *tx)
    .await?;

    let items = sqlx::query_as::<_, crate::sales_history::SaleRow>(
        "select o.id,
                o.receipt_prefix || '-' || lpad(o.receipt_number::text, 6, '0') as receipt_number,
                o.created_at,
                (select p.created_by from payments p
                  where p.order_id = o.id order by p.created_at, p.id limit 1) as cashier_id,
                (select u.full_name from payments p join users u on u.id = p.created_by
                  where p.order_id = o.id order by p.created_at, p.id limit 1) as cashier_name,
                coalesce((select c.name from customers c where c.id = o.customer_id),
                         o.customer_alias, 'Consumidor final') as customer_label,
                o.total_amount as total,
                o.refunded_amount,
                coalesce((select array_agg(distinct pm.tima_codigo order by pm.tima_codigo)
                            from payments p join tbl_tipos_maestra pm on pm.tima_id = p.method_id
                           where p.order_id = o.id), '{}') as methods,
                st.tima_codigo as status,
                (select ft.tima_codigo from fiscal_documents f
                   join tbl_tipos_maestra ft on ft.tima_id = f.status_id
                  where f.order_id = o.id and f.doc_type = 'POS' and f.refund_id is null) as fiscal_status
           from orders o
           join tbl_tipos_maestra st on st.tima_id = o.status_id
          where o.tenant_id = $1 and o.customer_id = $2
            and o.receipt_number is not null
            and st.tima_codigo in ('COBRADA', 'CERRADA', 'CANCELADA')
          order by o.created_at desc
          limit $3 offset $4",
    )
    .bind(scope.tenant)
    .bind(customer_id)
    .bind(page_size)
    .bind(offset)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(crate::sales_history::HistoryPage {
        items,
        total_count: count,
        totals: crate::sales_history::Totals { count, amount },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_customer_splits_dv() {
        // Caso 1: "900.123.456-8" -> número 900123456, DV 8, person_type JURIDICA
        let input1 = CreateCustomerInput {
            doc_type: Some("NIT".into()),
            doc_number: Some("900.123.456-8".into()),
            dv: None,
            name: "Empresa Uno SAS".into(),
            phone: None,
            email: None,
            price_tier_id: None,
            tax_regime: None,
            fiscal_responsibilities: None,
            ciiu: None,
            withholding_agent: None,
            city_code: None,
            billing_address: None,
            shipping_address: None,
            pinned_note: None,
            credit_limit: None,
            credit_block_days: None,
        };
        let norm1 = normalize_customer(input1).unwrap();
        assert_eq!(norm1.doc_type.as_deref(), Some("NIT"));
        assert_eq!(norm1.doc_number.as_deref(), Some("900123456"));
        assert_eq!(norm1.dv.as_deref(), Some("8"));
        assert_eq!(norm1.person_type, "JURIDICA");

        // Caso 2: "900123456-7" con dv = 8 -> CUSTOMER_DV_CONFLICT
        let input2 = CreateCustomerInput {
            doc_type: Some("NIT".into()),
            doc_number: Some("900123456-7".into()),
            dv: Some("8".into()),
            name: "Empresa Conflicto".into(),
            phone: None,
            email: None,
            price_tier_id: None,
            tax_regime: None,
            fiscal_responsibilities: None,
            ciiu: None,
            withholding_agent: None,
            city_code: None,
            billing_address: None,
            shipping_address: None,
            pinned_note: None,
            credit_limit: None,
            credit_block_days: None,
        };
        let err2 = normalize_customer(input2);
        match err2 {
            Err(ApiError::Domain(DomainError::Coded { code, status, .. })) => {
                assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
                assert_eq!(code, "CUSTOMER_DV_CONFLICT");
            }
            other => panic!("esperaba CUSTOMER_DV_CONFLICT pero obtuve {:?}", other),
        }

        // Caso 3: "cc" -> CC, person_type NATURAL, dv null
        let input3 = CreateCustomerInput {
            doc_type: Some("cc".into()),
            doc_number: Some(" 1.020.304-A ".into()),
            dv: None,
            name: "Juan Pérez".into(),
            phone: None,
            email: None,
            price_tier_id: None,
            tax_regime: None,
            fiscal_responsibilities: None,
            ciiu: None,
            withholding_agent: None,
            city_code: None,
            billing_address: None,
            shipping_address: None,
            pinned_note: None,
            credit_limit: None,
            credit_block_days: None,
        };
        let norm3 = normalize_customer(input3).unwrap();
        assert_eq!(norm3.doc_type.as_deref(), Some("CC"));
        assert_eq!(norm3.person_type, "NATURAL");
        assert_eq!(norm3.dv, None);
    }

    #[test]
    fn compute_missing_fiscal_checks_order() {
        let missing = compute_missing_fiscal(None, None, None, None, None);
        assert_eq!(
            missing,
            vec![
                "documento".to_string(),
                "correo".to_string(),
                "municipio".to_string(),
                "dirección de facturación".to_string(),
            ]
        );

        let partial =
            compute_missing_fiscal(Some("CC"), Some("12345678"), None, Some("05001"), None);
        assert_eq!(
            partial,
            vec!["correo".to_string(), "dirección de facturación".to_string()]
        );
    }
}
