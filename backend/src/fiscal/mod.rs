//! Facturación electrónica: documento equivalente POS electrónico y notas
//! crédito POS.
//!
//! La venta (y la devolución aprobada) deja su documento en `fiscal_documents`
//! como PENDIENTE, en la misma transacción. Un proceso de fondo
//! ([`spawn_worker`]) lo toma, lo arma con los datos de la venta, lo envía al
//! proveedor de la sede y guarda el CUDE, el QR y el PDF. Si el proveedor no
//! responde, reintenta con espera creciente; si la DIAN lo rechaza, queda
//! RECHAZADO con el motivo y se puede reintentar desde la API.
//!
//! Proveedores: `SIMULADO` (no sale a internet, para desarrollo y demos) y
//! `MATIAS` (MATIAS API, sandbox o producción según `MATIAS_API_URL`).

pub mod matias;

#[cfg(test)]
mod db_tests;

use std::time::Duration;

use axum::extract::State;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha384};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::auth::AuthUser;
use crate::error::{self, ApiError, DomainError};
use crate::routes::{Body, Path, Query};
use crate::tenancy::{self, Action};
use crate::{AppState, db};

type Tx<'a> = Transaction<'a, Postgres>;
type ApiResult<T> = Result<T, ApiError>;
type CustomerRow = (String, Option<String>, Option<String>, Option<String>);
type OriginalRow = (String, i64, DateTime<Utc>, Option<String>, String);
type NumberingRow = (
    String,
    Option<String>,
    i64,
    Option<i64>,
    Option<NaiveDate>,
    Option<NaiveDate>,
);

/// Intentos antes de dejar un documento quieto para revisión manual.
const MAX_ATTEMPTS: i32 = 12;

// ------------------------------------------------------------------ modelo

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub code: String,
    pub description: String,
    pub qty: Decimal,
    /// Base después de descuentos, sin impuesto.
    pub line_total: Decimal,
    pub tax_rate: Decimal,
    pub tax_amount: Decimal,
    pub tax_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Payment {
    /// Código del medio en Nexa: EFECTIVO, DATAFONO, QR, CREDITO, SALDO.
    pub method: String,
    pub amount: Decimal,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Customer {
    pub name: String,
    pub doc_type: Option<String>,
    pub doc_number: String,
    pub email: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PosDoc {
    pub id: Uuid,
    pub resolution_number: String,
    pub prefix: String,
    pub number: i64,
    pub cashier: String,
    pub terminal: String,
    pub address: String,
    pub notes: Option<String>,
    pub customer: Option<Customer>,
    pub lines: Vec<Line>,
    pub payments: Vec<Payment>,
}

#[derive(Debug, Clone)]
pub struct CreditNote {
    pub id: Uuid,
    pub resolution_number: String,
    pub prefix: String,
    pub number: i64,
    pub reason: String,
    pub ref_prefix: String,
    pub ref_number: i64,
    pub ref_date: NaiveDate,
    pub ref_cude: String,
    /// Devuelve toda la venta (anulación) o una parte.
    pub full: bool,
    pub customer: Option<Customer>,
    pub lines: Vec<Line>,
    pub payments: Vec<Payment>,
}

/// Resultado de un envío o consulta al proveedor.
#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Accepted {
        cude: String,
        provider_uuid: Option<String>,
        qr_url: Option<String>,
        pdf_url: Option<String>,
    },
    /// La DIAN lo está procesando: se consulta de nuevo.
    InProcess { cude: Option<String> },
    /// Rechazado por la DIAN o por el proveedor (datos): no se reintenta solo.
    Rejected { reason: String },
    /// Puede que ya se haya enviado: consultar antes de reenviar.
    CheckStatus { error: String },
    /// Falla temporal (red, proveedor caído, credenciales): se reintenta.
    Retry { error: String },
}

/// Proveedores disponibles en este backend.
#[derive(Clone, Default)]
pub struct Providers {
    pub matias: Option<matias::Client>,
}

impl Providers {
    pub fn from_env() -> Self {
        Self {
            matias: matias::Client::from_env(),
        }
    }
}

/// CUDE de mentira para el modo SIMULADO (SHA-384, mismo largo que el real).
fn simulated_cude(tenant: Uuid, prefix: &str, number: i64, total: Decimal) -> String {
    let digest = Sha384::digest(format!("SIMULADO|{tenant}|{prefix}{number}|{total}").as_bytes());
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

// ------------------------------------------------------------------ datos

#[derive(sqlx::FromRow)]
struct DocRow {
    tenant_id: Uuid,
    branch_id: Uuid,
    order_id: Uuid,
    refund_id: Option<Uuid>,
    doc_type: String,
    prefix: String,
    number: i64,
    status: String,
    attempts: i32,
}

#[derive(sqlx::FromRow)]
struct SettingsRow {
    provider: String,
    provider_client_id: Option<String>,
    pos_address: Option<String>,
}

async fn load_lines_for_order(tx: &mut Tx<'_>, order: Uuid) -> Result<Vec<Line>, sqlx::Error> {
    let rows: Vec<(
        String,
        String,
        Decimal,
        Decimal,
        Decimal,
        Decimal,
        Option<String>,
        Option<Decimal>,
        Option<Decimal>,
    )> = sqlx::query_as(
        "select coalesce(p.sku, left(coalesce(oi.product_id::text, 'LIBRE'), 8)),
                coalesce(oi.custom_name, p.name),
                oi.qty, oi.line_total, oi.tax_rate, oi.tax_amount,
                tx.tima_codigo, p.alcohol_degrees, p.volume_ml
           from order_items oi
           left join products p on p.id = oi.product_id
           left join tbl_tipos_maestra tx on tx.tima_id = p.tax_type_id
          where oi.order_id = $1 and oi.line_total > 0
          order by oi.created_at, oi.id",
    )
    .bind(order)
    .fetch_all(&mut **tx)
    .await?;
    Ok(rows
        .into_iter()
        .map(
            |(
                code,
                description,
                qty,
                line_total,
                tax_rate,
                tax_amount,
                tax_code,
                degrees,
                volume,
            )| {
                let is_icl = tax_code.as_deref() == Some("ICL")
                    || (degrees.is_some_and(|d| d > Decimal::ZERO)
                        && volume.is_some_and(|v| v > Decimal::ZERO));
                let (tax_id, calculated_tax_amount) = if is_icl {
                    let deg = degrees.unwrap_or(Decimal::ZERO);
                    let vol = volume.unwrap_or(Decimal::ZERO);
                    let icl_unit = crate::catalog::calculate_icl(deg, vol);
                    let total_icl = (icl_unit * qty).round_dp_with_strategy(
                        2,
                        rust_decimal::RoundingStrategy::MidpointAwayFromZero,
                    );
                    (
                        "18".to_string(),
                        if tax_amount.is_zero() {
                            total_icl
                        } else {
                            tax_amount
                        },
                    )
                } else {
                    let (tid, _) = matias::tax_of(tax_rate);
                    (tid.to_string(), tax_amount)
                };
                Line {
                    code,
                    description,
                    qty,
                    line_total,
                    tax_rate,
                    tax_amount: calculated_tax_amount,
                    tax_id,
                }
            },
        )
        .collect())
}

async fn load_customer(tx: &mut Tx<'_>, order: Uuid) -> Result<Option<Customer>, sqlx::Error> {
    let row: Option<CustomerRow> = sqlx::query_as(
        "select c.name, c.doc_type, c.doc_number, c.email
           from orders o join customers c on c.id = o.customer_id
          where o.id = $1",
    )
    .bind(order)
    .fetch_optional(&mut **tx)
    .await?;
    // Sin documento de identidad es consumidor final.
    Ok(row.and_then(|(name, doc_type, doc_number, email)| {
        doc_number.map(|n| Customer {
            name,
            doc_type,
            doc_number: n,
            email,
        })
    }))
}

async fn resolution(
    tx: &mut Tx<'_>,
    branch: Uuid,
    doc_type: &str,
) -> Result<Option<String>, sqlx::Error> {
    sqlx::query_scalar(
        "select resolution_number from document_sequences where branch_id = $1 and doc_type = $2",
    )
    .bind(branch)
    .bind(doc_type)
    .fetch_optional(&mut **tx)
    .await
    .map(Option::flatten)
}

enum Built {
    Pos(PosDoc),
    Note(CreditNote),
}

/// Arma el documento a partir de la venta o la devolución. `Err(texto)` =
/// falta algo para poder enviarlo (se reintenta más tarde).
async fn build(
    tx: &mut Tx<'_>,
    id: Uuid,
    d: &DocRow,
    s: &SettingsRow,
) -> Result<Result<Built, String>, sqlx::Error> {
    let (cashier, terminal): (Option<String>, Option<String>) = sqlx::query_as(
        "select u.full_name, r.code
           from orders o
           left join cash_sessions cs on cs.id = o.cash_session_id
           left join users u on u.id = cs.opened_by
           left join cash_registers r on r.id = cs.register_id
          where o.id = $1",
    )
    .bind(d.order_id)
    .fetch_optional(&mut **tx)
    .await?
    .unwrap_or((None, None));
    let branch_name: String = sqlx::query_scalar("select name from branches where id = $1")
        .bind(d.branch_id)
        .fetch_one(&mut **tx)
        .await?;
    let customer = load_customer(tx, d.order_id).await?;
    let payments: Vec<Payment> = sqlx::query_as::<_, (String, Decimal)>(
        "select t.tima_codigo, p.amount from payments p join tbl_tipos_maestra t on t.tima_id = p.method_id
          where p.order_id = $1 order by p.created_at",
    )
    .bind(d.order_id)
    .fetch_all(&mut **tx)
    .await?
    .into_iter()
    .map(|(method, amount)| Payment { method, amount })
    .collect();
    let needs_resolution = s.provider != "SIMULADO";

    if d.doc_type == "POS" {
        let res = resolution(tx, d.branch_id, "POS").await?;
        if needs_resolution && res.is_none() {
            return Ok(Err(
                "falta cargar la resolución de numeración POS de la DIAN".into(),
            ));
        }
        let lines = load_lines_for_order(tx, d.order_id).await?;
        if lines.is_empty() {
            return Ok(Err("la venta no tiene líneas con valor".into()));
        }
        return Ok(Ok(Built::Pos(PosDoc {
            id,
            resolution_number: res.unwrap_or_default(),
            prefix: d.prefix.clone(),
            number: d.number,
            cashier: cashier.unwrap_or_else(|| "Caja".into()),
            terminal: terminal.unwrap_or_else(|| "CAJA-1".into()),
            address: s
                .pos_address
                .clone()
                .filter(|a| !a.is_empty())
                .unwrap_or(branch_name),
            notes: None,
            customer,
            lines,
            payments,
        })));
    }

    // Nota crédito de una devolución.
    let Some(refund) = d.refund_id else {
        return Ok(Err("nota crédito sin devolución".into()));
    };
    let res = resolution(tx, d.branch_id, "DEVOLUCION").await?;
    if needs_resolution && res.is_none() {
        return Ok(Err("falta cargar la numeración de notas crédito POS".into()));
    }
    let original: Option<OriginalRow> = sqlx::query_as(
        "select f.prefix, f.number, f.created_at, f.cude, t.tima_codigo
           from fiscal_documents f join tbl_tipos_maestra t on t.tima_id = f.status_id
          where f.order_id = $1 and f.doc_type = 'POS' and f.refund_id is null",
    )
    .bind(d.order_id)
    .fetch_optional(&mut **tx)
    .await?;
    let Some((ref_prefix, ref_number, ref_at, Some(ref_cude), status)) = original else {
        return Ok(Err(
            "esperando que la DIAN acepte el documento POS original".into(),
        ));
    };
    if status != "ACEPTADO" {
        return Ok(Err(
            "esperando que la DIAN acepte el documento POS original".into(),
        ));
    }
    let rows: Vec<(
        String,
        String,
        Decimal,
        Decimal,
        Decimal,
        Decimal,
        Option<String>,
        Option<Decimal>,
        Option<Decimal>,
    )> = sqlx::query_as(
        "select coalesce(p.sku, left(coalesce(oi.product_id::text, 'LIBRE'), 8)), coalesce(oi.custom_name, p.name),
                ri.qty, ri.line_total, oi.tax_rate, ri.tax_amount,
                tx.tima_codigo, p.alcohol_degrees, p.volume_ml
           from refund_items ri
           join order_items oi on oi.id = ri.order_item_id
           left join products p on p.id = oi.product_id
           left join tbl_tipos_maestra tx on tx.tima_id = p.tax_type_id
          where ri.refund_id = $1 and ri.line_total > 0",
    )
    .bind(refund)
    .fetch_all(&mut **tx)
    .await?;
    let lines: Vec<Line> = rows
        .into_iter()
        .map(
            |(
                code,
                description,
                qty,
                line_total,
                tax_rate,
                tax_amount,
                tax_code,
                degrees,
                volume,
            )| {
                let is_icl = tax_code.as_deref() == Some("ICL")
                    || (degrees.is_some_and(|d| d > Decimal::ZERO)
                        && volume.is_some_and(|v| v > Decimal::ZERO));
                let (tax_id, calculated_tax_amount) = if is_icl {
                    let deg = degrees.unwrap_or(Decimal::ZERO);
                    let vol = volume.unwrap_or(Decimal::ZERO);
                    let icl_unit = crate::catalog::calculate_icl(deg, vol);
                    let total_icl = (icl_unit * qty).round_dp_with_strategy(
                        2,
                        rust_decimal::RoundingStrategy::MidpointAwayFromZero,
                    );
                    (
                        "18".to_string(),
                        if tax_amount.is_zero() {
                            total_icl
                        } else {
                            tax_amount
                        },
                    )
                } else {
                    let (tid, _) = matias::tax_of(tax_rate);
                    (tid.to_string(), tax_amount)
                };
                Line {
                    code,
                    description,
                    qty,
                    line_total,
                    tax_rate,
                    tax_amount: calculated_tax_amount,
                    tax_id,
                }
            },
        )
        .collect();
    let (reason, method, refund_total, order_total): (Option<String>, Option<String>, Decimal, Decimal) = sqlx::query_as(
        "select r.reason, r.method, r.total, o.total_amount from refunds r join orders o on o.id = r.order_id where r.id = $1",
    )
    .bind(refund)
    .fetch_one(&mut **tx)
    .await?;
    Ok(Ok(Built::Note(CreditNote {
        id,
        resolution_number: res.unwrap_or_default(),
        prefix: d.prefix.clone(),
        number: d.number,
        reason: reason.unwrap_or_else(|| "Devolución".into()),
        ref_prefix,
        ref_number,
        ref_date: ref_at.date_naive(),
        ref_cude,
        full: refund_total >= order_total,
        customer,
        lines,
        payments: vec![Payment {
            method: method.unwrap_or_else(|| "EFECTIVO".into()),
            amount: refund_total,
        }],
    })))
}

fn backoff(attempts: i32) -> chrono::Duration {
    // 30 s, 1 min, 2 min… hasta 1 hora.
    let secs = 30i64.saturating_mul(1i64 << attempts.clamp(0, 7));
    chrono::Duration::seconds(secs.min(3600))
}

/// Envía o consulta un documento. Devuelve el estado en que quedó.
pub async fn process_one(
    pool: &PgPool,
    providers: &Providers,
    id: Uuid,
    tenant: Uuid,
    branch: Uuid,
) -> anyhow::Result<String> {
    let mut tx = db::begin_system_tx(pool, tenant, Some(branch)).await?;
    let d: DocRow = sqlx::query_as(
        "select f.tenant_id, f.branch_id, f.order_id, f.refund_id, f.doc_type, f.prefix, f.number,
                t.tima_codigo as status, f.attempts
           from fiscal_documents f join tbl_tipos_maestra t on t.tima_id = f.status_id
          where f.id = $1 for update of f",
    )
    .bind(id)
    .fetch_one(&mut *tx)
    .await?;
    let s: SettingsRow = sqlx::query_as("select provider, provider_client_id, pos_address from fiscal_settings where branch_id = $1")
        .bind(d.branch_id)
        .fetch_one(&mut *tx)
        .await?;
    let built = build(&mut tx, id, &d, &s).await?;
    let res_type = if d.doc_type == "POS" {
        "POS"
    } else {
        "DEVOLUCION"
    };
    let resolution_number = resolution(&mut tx, d.branch_id, res_type)
        .await?
        .unwrap_or_default();
    tx.commit().await?;

    let client = s.provider_client_id.as_deref().filter(|c| !c.is_empty());
    let outcome = match (&built, s.provider.as_str()) {
        (Err(why), _) => Outcome::Retry { error: why.clone() },
        (Ok(b), "SIMULADO") => {
            let total = match b {
                Built::Pos(p) => p.lines.iter().map(|l| l.line_total + l.tax_amount).sum(),
                Built::Note(n) => n.lines.iter().map(|l| l.line_total + l.tax_amount).sum(),
            };
            Outcome::Accepted {
                cude: simulated_cude(d.tenant_id, &d.prefix, d.number, total),
                provider_uuid: None,
                qr_url: None,
                pdf_url: None,
            }
        }
        (Ok(b), _) => match &providers.matias {
            None => Outcome::Retry {
                error: "el backend no tiene MATIAS_API_TOKEN configurado".into(),
            },
            Some(m) if d.status == "EN_PROCESO" => {
                m.status(&d.prefix, d.number, &resolution_number, client)
                    .await
            }
            Some(m) => {
                let out = match b {
                    Built::Pos(p) => m.send_pos(&matias::pos_json(p), client).await,
                    Built::Note(n) => {
                        m.send_credit_note(&matias::credit_note_json(n), client)
                            .await
                    }
                };
                match out {
                    Outcome::CheckStatus { .. } => {
                        m.status(&d.prefix, d.number, &resolution_number, client)
                            .await
                    }
                    other => other,
                }
            }
        },
    };

    let mut tx = db::begin_system_tx(pool, tenant, Some(branch)).await?;
    let attempts = d.attempts + 1;
    let (status, cude, provider_uuid, qr, pdf, error, next) = match &outcome {
        Outcome::Accepted {
            cude,
            provider_uuid,
            qr_url,
            pdf_url,
        } => (
            "ACEPTADO",
            Some(cude.clone()),
            provider_uuid.clone(),
            qr_url.clone(),
            pdf_url.clone(),
            None,
            Utc::now(),
        ),
        Outcome::InProcess { cude } => (
            "EN_PROCESO",
            cude.clone(),
            None,
            None,
            None,
            Some("la DIAN lo está procesando".to_string()),
            Utc::now() + chrono::Duration::seconds(60),
        ),
        Outcome::Rejected { reason } => (
            "RECHAZADO",
            None,
            None,
            None,
            None,
            Some(reason.clone()),
            Utc::now(),
        ),
        Outcome::Retry { error } | Outcome::CheckStatus { error } => (
            "PENDIENTE",
            None,
            None,
            None,
            None,
            Some(error.clone()),
            Utc::now() + backoff(d.attempts),
        ),
    };
    sqlx::query(
        "update fiscal_documents
            set status_id = public.tipo_id('DOCF', $2), attempts = $3, last_error = $4, next_attempt_at = $5,
                cude = coalesce($6, cude), provider_uuid = coalesce($7, provider_uuid),
                qr_url = coalesce($8, qr_url), pdf_url = coalesce($9, pdf_url), provider = $10,
                sent_at = coalesce(sent_at, case when $2 <> 'PENDIENTE' then now() end),
                accepted_at = case when $2 = 'ACEPTADO' then now() else accepted_at end,
                updated_at = now()
          where id = $1",
    )
    .bind(id)
    .bind(status)
    .bind(attempts)
    .bind(&error)
    .bind(next)
    .bind(&cude)
    .bind(&provider_uuid)
    .bind(&qr)
    .bind(&pdf)
    .bind(&s.provider)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    if attempts >= MAX_ATTEMPTS && status == "PENDIENTE" {
        tracing::warn!(%id, error = ?error, "documento fiscal sin enviar tras varios intentos");
    }
    Ok(status.to_string())
}

/// Procesa los documentos que tocan. Devuelve cuántos tomó.
pub async fn process_due(
    pool: &PgPool,
    providers: &Providers,
    limit: i32,
) -> anyhow::Result<usize> {
    let due: Vec<(Uuid, Uuid, Uuid)> =
        sqlx::query_as("select id, tenant_id, branch_id from private.fiscal_claim_due($1)")
            .bind(limit)
            .fetch_all(pool)
            .await?;
    for (id, tenant, branch) in &due {
        if let Err(e) = process_one(pool, providers, *id, *tenant, *branch).await {
            tracing::error!(%id, error = %e, "falló el envío de un documento fiscal");
        }
    }
    Ok(due.len())
}

fn env_providers() -> &'static Providers {
    static P: std::sync::OnceLock<Providers> = std::sync::OnceLock::new();
    P.get_or_init(Providers::from_env)
}

fn worker_enabled() -> bool {
    !matches!(std::env::var("FISCAL_WORKER").as_deref(), Ok("0" | "false"))
}

/// Envía ya lo pendiente, sin esperar el ciclo: se llama al terminar una
/// venta o una devolución para que el tiquete salga con su CUDE y su QR.
pub fn kick(pool: &PgPool) {
    if !worker_enabled() {
        return;
    }
    let pool = pool.clone();
    tokio::spawn(async move {
        if let Err(e) = process_due(&pool, env_providers(), 20).await {
            tracing::error!(error = %e, "falló el envío inmediato de documentos fiscales");
        }
    });
}

/// Proceso de fondo: cada 10 s envía lo pendiente. Se apaga con `FISCAL_WORKER=0`.
pub fn spawn_worker(pool: PgPool) {
    if !worker_enabled() {
        return;
    }
    let providers = env_providers().clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(10));
        loop {
            tick.tick().await;
            if let Err(e) = process_due(&pool, &providers, 20).await {
                tracing::error!(error = %e, "falló el ciclo de facturación electrónica");
            }
        }
    });
}

// ------------------------------------------------------------------ API

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/v1/fiscal/settings", get(get_settings).put(put_settings))
        .route("/v1/fiscal/documents", get(list_documents))
        .route("/v1/fiscal/documents/{id}/retry", post(retry_document))
        .route("/v1/fiscal/orders/{order_id}", get(order_document))
        .route("/v1/fiscal/test", post(test_connection))
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct Numbering {
    pub prefix: String,
    pub resolution_number: Option<String>,
    /// Próximo número a usar (solo al cambiar de resolución).
    pub next_number: Option<i64>,
    pub range_to: Option<i64>,
    pub resolution_date: Option<NaiveDate>,
    pub valid_until: Option<NaiveDate>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct FiscalSettings {
    pub branch_id: Uuid,
    pub provider: String,
    pub enabled: bool,
    pub provider_client_id: Option<String>,
    pub pos_address: Option<String>,
    pub pos: Option<Numbering>,
    pub credit_notes: Option<Numbering>,
}

#[derive(Deserialize)]
struct BranchQuery {
    branch_id: Uuid,
}

async fn numbering(
    tx: &mut Tx<'_>,
    branch: Uuid,
    doc_type: &str,
) -> Result<Option<Numbering>, sqlx::Error> {
    let row: Option<NumberingRow> = sqlx::query_as(
        "select prefix, resolution_number, next_number, range_to, resolution_date, valid_until
           from document_sequences where branch_id = $1 and doc_type = $2",
    )
    .bind(branch)
    .bind(doc_type)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.map(
        |(prefix, resolution_number, next, range_to, resolution_date, valid_until)| Numbering {
            prefix,
            resolution_number,
            next_number: Some(next),
            range_to,
            resolution_date,
            valid_until,
        },
    ))
}

pub async fn settings(
    pool: &PgPool,
    claims: &crate::auth::Claims,
    branch: Uuid,
) -> ApiResult<FiscalSettings> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, branch).await?;
    scope.require(Action::Read)?;
    let s: Option<(String, bool, Option<String>, Option<String>)> =
        sqlx::query_as("select provider, enabled, provider_client_id, pos_address from fiscal_settings where branch_id = $1")
            .bind(branch)
            .fetch_optional(&mut *tx)
            .await?;
    let pos = numbering(&mut tx, branch, "POS").await?;
    let credit_notes = numbering(&mut tx, branch, "DEVOLUCION").await?;
    tx.commit().await?;
    let (provider, enabled, provider_client_id, pos_address) =
        s.unwrap_or(("SIMULADO".into(), false, None, None));
    Ok(FiscalSettings {
        branch_id: branch,
        provider,
        enabled,
        provider_client_id,
        pos_address,
        pos,
        credit_notes,
    })
}

fn validate(s: &FiscalSettings) -> ApiResult<()> {
    if !matches!(s.provider.as_str(), "SIMULADO" | "MATIAS") {
        return Err(error::validation("provider debe ser SIMULADO o MATIAS"));
    }
    for (name, n) in [("pos", &s.pos), ("credit_notes", &s.credit_notes)] {
        let Some(n) = n else { continue };
        let p = n.prefix.trim();
        if p.is_empty() || p.len() > 4 || !p.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err(error::validation(format!(
                "{name}.prefix: de 1 a 4 letras o números"
            )));
        }
        if n.resolution_number
            .as_deref()
            .is_some_and(|r| !r.chars().all(|c| c.is_ascii_digit()) || r.is_empty())
        {
            return Err(error::validation(format!(
                "{name}.resolution_number: solo números"
            )));
        }
        if n.next_number.is_some_and(|x| x < 1) || n.range_to.is_some_and(|x| x < 1) {
            return Err(error::validation(format!("{name}: números positivos")));
        }
        if let (Some(a), Some(b)) = (n.next_number, n.range_to)
            && a > b
        {
            return Err(error::validation(format!(
                "{name}: el próximo número está fuera del rango autorizado"
            )));
        }
    }
    if s.enabled
        && s.provider == "MATIAS"
        && s.pos
            .as_ref()
            .and_then(|p| p.resolution_number.as_ref())
            .is_none()
    {
        return Err(error::validation(
            "para enviar a MATIAS falta la resolución de numeración POS",
        ));
    }
    Ok(())
}

async fn upsert_numbering(
    tx: &mut Tx<'_>,
    tenant: Uuid,
    branch: Uuid,
    doc_type: &str,
    n: &Numbering,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "insert into document_sequences (tenant_id, branch_id, doc_type, prefix, next_number, range_to, resolution_number, resolution_date, valid_until)
         values ($1, $2, $3, $4, coalesce($5, 1), $6, $7, $8, $9)
         on conflict (branch_id, doc_type) do update set
            prefix = excluded.prefix, range_to = excluded.range_to, resolution_number = excluded.resolution_number,
            resolution_date = excluded.resolution_date, valid_until = excluded.valid_until,
            next_number = coalesce($5, document_sequences.next_number)",
    )
    .bind(tenant)
    .bind(branch)
    .bind(doc_type)
    .bind(n.prefix.trim().to_uppercase())
    .bind(n.next_number)
    .bind(n.range_to)
    .bind(&n.resolution_number)
    .bind(n.resolution_date)
    .bind(n.valid_until)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn save_settings(
    pool: &PgPool,
    claims: &crate::auth::Claims,
    s: &FiscalSettings,
) -> ApiResult<()> {
    validate(s)?;
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, s.branch_id).await?;
    if !scope.is_admin() {
        return Err(ApiError::Domain(DomainError::Forbidden));
    }
    sqlx::query(
        "insert into fiscal_settings (tenant_id, branch_id, provider, enabled, provider_client_id, pos_address)
         values ($1, $2, $3, $4, $5, $6)
         on conflict (branch_id) do update set provider = excluded.provider, enabled = excluded.enabled,
            provider_client_id = excluded.provider_client_id, pos_address = excluded.pos_address, updated_at = now()",
    )
    .bind(scope.tenant)
    .bind(s.branch_id)
    .bind(&s.provider)
    .bind(s.enabled)
    .bind(s.provider_client_id.as_deref().map(str::trim).filter(|c| !c.is_empty()))
    .bind(s.pos_address.as_deref().map(str::trim).filter(|c| !c.is_empty()))
    .execute(&mut *tx)
    .await?;
    if let Some(n) = &s.pos {
        upsert_numbering(&mut tx, scope.tenant, s.branch_id, "POS", n).await?;
    }
    if let Some(n) = &s.credit_notes {
        upsert_numbering(&mut tx, scope.tenant, s.branch_id, "DEVOLUCION", n).await?;
    }
    crate::audit::record_tx(
        &mut tx,
        &scope,
        "UPDATE",
        "fiscal_settings",
        s.branch_id,
        None,
        Some(serde_json::json!({ "provider": s.provider, "enabled": s.enabled })),
        None,
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

async fn get_settings(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<BranchQuery>,
) -> ApiResult<Json<FiscalSettings>> {
    Ok(Json(
        settings(&state.pool, &user.claims, q.branch_id).await?,
    ))
}

async fn put_settings(
    State(state): State<AppState>,
    user: AuthUser,
    Body(s): Body<FiscalSettings>,
) -> ApiResult<Json<FiscalSettings>> {
    save_settings(&state.pool, &user.claims, &s).await?;
    Ok(Json(
        settings(&state.pool, &user.claims, s.branch_id).await?,
    ))
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct FiscalDocument {
    pub id: Uuid,
    pub order_id: Uuid,
    pub refund_id: Option<Uuid>,
    pub doc_type: String,
    pub prefix: String,
    pub number: i64,
    pub status: String,
    pub provider: Option<String>,
    pub cude: Option<String>,
    pub qr_url: Option<String>,
    pub pdf_url: Option<String>,
    pub attempts: i32,
    pub last_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub accepted_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
struct DocsQuery {
    branch_id: Uuid,
    status: Option<String>,
}

async fn list_documents(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<DocsQuery>,
) -> ApiResult<Json<Vec<FiscalDocument>>> {
    let (scope, mut tx) = tenancy::begin_in_branch(&state.pool, &user.claims, q.branch_id).await?;
    scope.require(Action::Read)?;
    let rows = sqlx::query_as::<_, FiscalDocument>(
        "select f.id, f.order_id, f.refund_id, f.doc_type, f.prefix, f.number, t.tima_codigo as status, f.provider,
                f.cude, f.qr_url, f.pdf_url, f.attempts, f.last_error, f.created_at, f.accepted_at
           from fiscal_documents f join tbl_tipos_maestra t on t.tima_id = f.status_id
          where f.branch_id = $1 and ($2::text is null or t.tima_codigo = $2)
          order by f.created_at desc limit 200",
    )
    .bind(q.branch_id)
    .bind(&q.status)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Json(rows))
}

/// Vuelve a poner en cola un documento rechazado o detenido (tras corregir datos).
pub async fn retry(pool: &PgPool, claims: &crate::auth::Claims, id: Uuid) -> ApiResult<()> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    if !scope.is_admin() {
        return Err(ApiError::Domain(DomainError::Forbidden));
    }
    let n = sqlx::query(
        "update fiscal_documents
            set status_id = public.tipo_id('DOCF', 'PENDIENTE'), attempts = 0, next_attempt_at = now(), updated_at = now()
          where id = $1 and status_id in (public.tipo_id('DOCF', 'RECHAZADO'), public.tipo_id('DOCF', 'PENDIENTE'))",
    )
    .bind(id)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if n == 0 {
        return Err(error::state(
            "solo se reintentan documentos pendientes o rechazados",
        ));
    }
    tx.commit().await?;
    Ok(())
}

async fn retry_document(
    State(state): State<AppState>,
    user: AuthUser,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<serde_json::Value>> {
    retry(&state.pool, &user.claims, id).await?;
    Ok(Json(serde_json::json!({ "status": "PENDIENTE" })))
}

/// Documento POS de una venta, con el QR listo para imprimir en el tiquete.
#[derive(Debug, Serialize)]
pub struct OrderDocument {
    pub id: Uuid,
    pub status: String,
    pub provider: Option<String>,
    pub number: String,
    pub cude: Option<String>,
    pub qr_url: Option<String>,
    /// QR de `qr_url` en SVG (solo cuando la DIAN ya lo aceptó).
    pub qr_svg: Option<String>,
    pub pdf_url: Option<String>,
    pub last_error: Option<String>,
}

type OrderDocRow = (
    Uuid,
    String,
    Option<String>,
    String,
    i64,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
);

pub async fn document_of_order(
    pool: &PgPool,
    claims: &crate::auth::Claims,
    order: Uuid,
) -> ApiResult<Option<OrderDocument>> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require(Action::Read)?;
    let row: Option<OrderDocRow> = sqlx::query_as(
        "select f.id, t.tima_codigo, f.provider, f.prefix, f.number, f.cude, f.qr_url, f.pdf_url, f.last_error
           from fiscal_documents f join tbl_tipos_maestra t on t.tima_id = f.status_id
          where f.order_id = $1 and f.doc_type = 'POS' and f.refund_id is null",
    )
    .bind(order)
    .fetch_optional(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(row.map(
        |(id, status, provider, prefix, number, cude, qr_url, pdf_url, last_error)| OrderDocument {
            id,
            qr_svg: qr_url
                .as_deref()
                .filter(|_| status == "ACEPTADO")
                .and_then(crate::whatsapp::qr_svg),
            number: format!("{prefix}-{number}"),
            status,
            provider,
            cude,
            qr_url,
            pdf_url,
            last_error,
        },
    ))
}

async fn order_document(
    State(state): State<AppState>,
    user: AuthUser,
    Path(order_id): Path<Uuid>,
) -> ApiResult<Json<Option<OrderDocument>>> {
    Ok(Json(
        document_of_order(&state.pool, &user.claims, order_id).await?,
    ))
}

/// Comprueba que el proveedor de la sede responde con las credenciales del backend.
async fn test_connection(
    State(state): State<AppState>,
    user: AuthUser,
    Query(q): Query<BranchQuery>,
) -> ApiResult<Json<serde_json::Value>> {
    let s = settings(&state.pool, &user.claims, q.branch_id).await?;
    let (ok, message) = match s.provider.as_str() {
        "SIMULADO" => (
            true,
            "Modo simulado: no se envía nada a la DIAN.".to_string(),
        ),
        _ => match &env_providers().matias {
            None => (false, "Falta MATIAS_API_TOKEN en el backend.".to_string()),
            Some(m) => m.ping(s.provider_client_id.as_deref()).await,
        },
    };
    Ok(Json(
        serde_json::json!({ "ok": ok, "provider": s.provider, "message": message }),
    ))
}
