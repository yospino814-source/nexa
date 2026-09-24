use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::Claims;
use crate::db;
use crate::error::{ApiError, DomainError};

/// Resuelve la pertenencia del usuario (rol y tenant) usando `memberships`
/// como fuente de verdad. Si el usuario no indica tenant, toma el suyo.
pub async fn resolve_membership(
    pool: &PgPool,
    claims: &Claims,
    requested_tenant: Option<Uuid>,
) -> Result<(Uuid, String), ApiError> {
    let mut tx = db::begin_tenant_tx(pool, claims).await?;

    let found = match requested_tenant.or_else(|| claims.tenant_id()) {
        Some(tenant) => {
            sqlx::query_as::<_, (Uuid, String)>(
                "select tenant_id, role from memberships
                 where user_id = auth.uid() and tenant_id = $1",
            )
            .bind(tenant)
            .fetch_optional(&mut *tx)
            .await?
        }
        None => {
            sqlx::query_as::<_, (Uuid, String)>(
                "select tenant_id, role from memberships
                 where user_id = auth.uid()
                 order by created_at
                 limit 1",
            )
            .fetch_optional(&mut *tx)
            .await?
        }
    };

    tx.commit().await?;
    found.ok_or(ApiError::Domain(DomainError::Forbidden))
}

/// Verifica que una sucursal pertenece al tenant indicado.
pub async fn ensure_location_in_tenant(
    pool: &PgPool,
    claims: &Claims,
    tenant_id: Uuid,
    location_id: Uuid,
) -> Result<(), ApiError> {
    let mut tx = db::begin_tenant_tx(pool, claims).await?;
    let exists: bool = sqlx::query_scalar(
        "select exists(select 1 from locations where tenant_id = $1 and id = $2)",
    )
    .bind(tenant_id)
    .bind(location_id)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;

    if exists {
        Ok(())
    } else {
        Err(ApiError::Domain(DomainError::Forbidden))
    }
}

/// Autoriza una operacion segun el rol del usuario dentro del tenant.
pub fn authorize(role: &str, allowed: &[&str]) -> Result<(), ApiError> {
    if allowed.contains(&role) {
        Ok(())
    } else {
        Err(ApiError::Domain(DomainError::Forbidden))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn authorize_allows_and_forbids() {
        assert!(authorize("admin_caja", &["admin_caja", "mesero"]).is_ok());
        assert!(authorize("cocina", &["admin_caja"]).is_err());
    }
}
