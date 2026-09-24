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
    use crate::test_support::*;

    fn database_url() -> Option<String> {
        dotenvy::dotenv().ok();
        std::env::var("DATABASE_URL").ok()
    }

    #[test]
    fn authorize_allows_and_forbids() {
        assert!(authorize("admin_caja", &["admin_caja", "mesero"]).is_ok());
        assert!(authorize("cocina", &["admin_caja"]).is_err());
    }

    #[tokio::test]
    async fn resolve_membership_from_claim_and_memberships() {
        let Some(admin) = admin_pool().await else {
            return;
        };
        let url = database_url().unwrap();
        let app = db::connect(&url).await.unwrap();

        let tenant = seed_tenant(&admin).await;
        let other = seed_tenant(&admin).await;
        let user = seed_user(&admin).await;
        seed_membership(&admin, tenant, user, "mesero").await;

        // Claim con el tenant correcto.
        let c = claims(user, Some(tenant));
        assert_eq!(
            resolve_membership(&app, &c, None).await.unwrap(),
            (tenant, "mesero".to_string())
        );

        // Tenant ajeno (sin pertenencia) -> forbidden.
        let c_other = claims(user, Some(other));
        assert!(resolve_membership(&app, &c_other, None).await.is_err());

        // Sin tenant en el claim -> resuelve el correcto.
        let c_none = claims(user, None);
        assert_eq!(
            resolve_membership(&app, &c_none, None).await.unwrap(),
            (tenant, "mesero".to_string())
        );

        cleanup(&admin, tenant, &[user]).await;
        cleanup(&admin, other, &[]).await;
    }

    #[tokio::test]
    async fn ensure_location_rejects_foreign_tenant() {
        let Some(admin) = admin_pool().await else {
            return;
        };
        let url = database_url().unwrap();
        let app = db::connect(&url).await.unwrap();

        let tenant = seed_tenant(&admin).await;
        let other = seed_tenant(&admin).await;
        let user = seed_user(&admin).await;
        seed_membership(&admin, tenant, user, "admin_caja").await;

        let loc = seed_location(&admin, tenant).await;
        let foreign = seed_location(&admin, other).await;

        let c = claims(user, Some(tenant));
        assert!(ensure_location_in_tenant(&app, &c, tenant, loc).await.is_ok());
        assert!(ensure_location_in_tenant(&app, &c, tenant, foreign)
            .await
            .is_err());

        cleanup(&admin, tenant, &[user]).await;
        cleanup(&admin, other, &[]).await;
    }
}
