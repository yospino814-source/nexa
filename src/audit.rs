//! Bitácora de auditoría inmutable (F0.5).
//!
//! Solo admite INSERT y SELECT. No admite UPDATE ni DELETE (protegido por trigger en BD).

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::ApiError;
use crate::tenancy::{self, Scope};

type Tx<'a> = Transaction<'a, Postgres>;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuditEntry {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub branch_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub user_name: Option<String>,
    pub action: String,
    pub entity: String,
    pub entity_id: Option<Uuid>,
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
    pub ip: Option<String>,
    pub at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct AuditFilter {
    pub branch_id: Option<Uuid>,
    pub action: Option<String>,
    pub entity: Option<String>,
    pub entity_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub from: Option<DateTime<Utc>>,
    pub to: Option<DateTime<Utc>>,
    pub limit: Option<i64>,
}

/// Registra una entrada en la bitácora de auditoría dentro de la transacción activa.
#[allow(clippy::too_many_arguments)]
pub async fn record_tx(
    tx: &mut Tx<'_>,
    scope: &Scope,
    action: &str,
    entity: &str,
    entity_id: Uuid,
    before: Option<serde_json::Value>,
    after: Option<serde_json::Value>,
    ip: Option<String>,
) -> Result<(), ApiError> {
    sqlx::query(
        "insert into audit_log (tenant_id, branch_id, user_id, action, entity, entity_id, before, after, ip, at)
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, now())",
    )
    .bind(scope.tenant)
    .bind(scope.branch)
    .bind(scope.user)
    .bind(action)
    .bind(entity)
    .bind(entity_id)
    .bind(before)
    .bind(after)
    .bind(ip)
    .execute(&mut **tx)
    .await?;

    Ok(())
}

/// Consulta entradas de auditoría con filtros.
pub async fn list_audit_entries(
    pool: &PgPool,
    claims: &Claims,
    filter: AuditFilter,
) -> Result<Vec<AuditEntry>, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("reports:read")?;

    let limit = filter.limit.unwrap_or(50).clamp(1, 200);

    let rows = sqlx::query_as::<_, AuditEntry>(
        "select a.id, a.tenant_id, a.branch_id, a.user_id, u.full_name as user_name,
                a.action, a.entity, a.entity_id, a.before, a.after, a.ip, a.at
           from audit_log a
           left join users u on u.id = a.user_id
          where a.tenant_id = $1
            and ($2::uuid is null or a.branch_id = $2)
            and ($3::text is null or a.action = $3)
            and ($4::uuid is null or a.user_id = $4)
            and ($5::timestamptz is null or a.at >= $5)
            and ($6::timestamptz is null or a.at <= $6)
            and ($7::text is null or a.entity = $7)
            and ($8::uuid is null or a.entity_id = $8)
          order by a.at desc
          limit $9",
    )
    .bind(scope.tenant)
    .bind(filter.branch_id)
    .bind(filter.action)
    .bind(filter.user_id)
    .bind(filter.from)
    .bind(filter.to)
    .bind(filter.entity)
    .bind(filter.entity_id)
    .bind(limit)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(rows)
}
