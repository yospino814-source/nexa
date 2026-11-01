#![allow(clippy::collapsible_if)]
//! Directorio de clientes mínimos para POS y facturación (F1.4).

use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{self, ApiError};
use crate::tenancy::{self, Scope};

type Tx<'a> = Transaction<'a, Postgres>;

const VALID_DOC_TYPES: &[&str] = &["CC", "NIT", "CE", "PP", "TI", "PEP", "PPT", "NIUP"];

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
}

#[derive(Debug, Clone, Deserialize)]
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
}

#[derive(Debug, Deserialize)]
pub struct CustomerQuery {
    #[serde(default)]
    pub q: Option<String>,
    #[serde(default)]
    pub limit: Option<i64>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct NormalizedCustomer {
    pub doc_type: Option<String>,
    pub doc_number: Option<String>,
    pub dv: Option<String>,
    pub name: String,
    pub phone: Option<String>,
    pub email: Option<String>,
}

/// Normalización pura antes de guardar o buscar.
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

    // 2. Doc number: mayúsculas, solo letras y dígitos
    let doc_number = input.doc_number.and_then(|raw| {
        let clean: String = raw
            .chars()
            .filter(|c| c.is_ascii_alphanumeric())
            .map(|c| c.to_ascii_uppercase())
            .collect();
        if clean.is_empty() { None } else { Some(clean) }
    });

    // 3. Doc type: mayúsculas y trim
    let doc_type = input.doc_type.and_then(|raw| {
        let clean = raw.trim().to_ascii_uppercase();
        if clean.is_empty() { None } else { Some(clean) }
    });

    // Validar correspondencia: si hay doc_number debe haber doc_type y viceversa
    if doc_number.is_some() != doc_type.is_some() {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "CUSTOMER_INVALID",
            "Tipo y número de documento deben proporcionarse juntos",
            None,
        ));
    }

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

    // 4. DV: solo permitido si doc_type == NIT
    let dv = input.dv.and_then(|raw| {
        let clean: String = raw.chars().filter(|c| c.is_ascii_digit()).collect();
        if clean.is_empty() { None } else { Some(clean) }
    });

    if dv.is_some() && doc_type.as_deref() != Some("NIT") {
        return Err(error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "CUSTOMER_INVALID",
            "El dígito de verificación (DV) solo aplica para NIT",
            None,
        ));
    }

    if let Some(ref d) = dv {
        if d.len() != 1 {
            return Err(error::coded(
                StatusCode::UNPROCESSABLE_ENTITY,
                "CUSTOMER_INVALID",
                "El DV debe ser un único dígito",
                None,
            ));
        }
    }

    // 5. Phone: solo dígitos, 7..15
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

    // 6. Email: minúsculas, trim, exactamente una '@'
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

    Ok(NormalizedCustomer {
        doc_type,
        doc_number,
        dv,
        name,
        phone,
        email,
    })
}

/// Busca clientes en el directorio del negocio.
pub async fn search(
    pool: &PgPool,
    claims: &Claims,
    query: CustomerQuery,
) -> Result<Vec<Customer>, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("customers:read")?;

    let limit = query.limit.unwrap_or(20).clamp(1, 50);
    let q = query.q.unwrap_or_default().trim().to_string();

    let rows = if q.is_empty() {
        sqlx::query_as::<_, Customer>(
            "select id, doc_type, doc_number, dv, name, phone, email, price_tier_id
               from customers
              where tenant_id = $1 and archived_at is null
              order by name asc
              limit $2",
        )
        .bind(scope.tenant)
        .bind(limit)
        .fetch_all(&mut *tx)
        .await?
    } else {
        let name_pattern = format!("%{q}%");
        let prefix_pattern = format!("{q}%");
        sqlx::query_as::<_, Customer>(
            "select id, doc_type, doc_number, dv, name, phone, email, price_tier_id
               from customers
              where tenant_id = $1
                and archived_at is null
                and (name ilike $2 or doc_number ilike $3 or phone ilike $3)
              order by name asc
              limit $4",
        )
        .bind(scope.tenant)
        .bind(name_pattern)
        .bind(prefix_pattern)
        .bind(limit)
        .fetch_all(&mut *tx)
        .await?
    };

    tx.commit().await?;
    Ok(rows)
}

/// Crea un cliente nuevo en el negocio.
pub async fn create(
    pool: &PgPool,
    claims: &Claims,
    input: CreateCustomerInput,
) -> Result<Customer, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("customers:write")?;

    let customer = create_tx(&mut tx, &scope, input).await?;
    tx.commit().await?;
    Ok(customer)
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
        "insert into customers (tenant_id, doc_type, doc_number, dv, name, phone, email)
         values ($1, $2, $3, $4, $5, $6, $7)
         returning id, doc_type, doc_number, dv, name, phone, email, price_tier_id",
    )
    .bind(scope.tenant)
    .bind(norm.doc_type)
    .bind(norm.doc_number)
    .bind(norm.dv)
    .bind(norm.name)
    .bind(norm.phone)
    .bind(norm.email)
    .fetch_one(&mut **tx)
    .await?;

    Ok(customer)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_customer_cleans_fields() {
        let input = CreateCustomerInput {
            doc_type: Some("cc".into()),
            doc_number: Some(" 1.020.304-A ".into()),
            dv: None,
            name: "  Juan   Carlos   Pérez  ".into(),
            phone: Some(" (300) 123-4567 ".into()),
            email: Some("  JUAN@EXAMPLE.COM  ".into()),
        };

        let norm = normalize_customer(input).unwrap();
        assert_eq!(norm.doc_type.as_deref(), Some("CC"));
        assert_eq!(norm.doc_number.as_deref(), Some("1020304A"));
        assert_eq!(norm.name, "Juan Carlos Pérez");
        assert_eq!(norm.phone.as_deref(), Some("3001234567"));
        assert_eq!(norm.email.as_deref(), Some("juan@example.com"));
        assert_eq!(norm.dv, None);

        // NIT con DV
        let nit_input = CreateCustomerInput {
            doc_type: Some("nit".into()),
            doc_number: Some("900.123.456".into()),
            dv: Some(" 1 ".into()),
            name: "Empresa SAS".into(),
            phone: None,
            email: None,
        };
        let norm_nit = normalize_customer(nit_input).unwrap();
        assert_eq!(norm_nit.doc_type.as_deref(), Some("NIT"));
        assert_eq!(norm_nit.doc_number.as_deref(), Some("900123456"));
        assert_eq!(norm_nit.dv.as_deref(), Some("1"));

        // Error: DV en no NIT
        let bad_dv = CreateCustomerInput {
            doc_type: Some("CC".into()),
            doc_number: Some("12345".into()),
            dv: Some("1".into()),
            name: "Test".into(),
            phone: None,
            email: None,
        };
        assert!(normalize_customer(bad_dv).is_err());

        // Error: Email con dos @
        let bad_email = CreateCustomerInput {
            doc_type: None,
            doc_number: None,
            dv: None,
            name: "Test".into(),
            phone: None,
            email: Some("a@b@c.com".into()),
        };
        assert!(normalize_customer(bad_email).is_err());
    }
}
