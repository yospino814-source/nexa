//! Permisos granulares y overrides por tenant (F0.1).
//!
//! Cada rol tiene una lista base de permisos (`role_permissions`). El dueño
//! de un negocio puede ajustar permisos para su tenant (`tenant_role_overrides`),
//! pero nunca sobre el rol `DUENO` ni sin tener rol `DUENO`.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{ApiError, DomainError, state};
use crate::tenancy;

type Tx<'a> = Transaction<'a, Postgres>;

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct Permission {
    pub code: String,
    pub module: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolePermissionItem {
    pub code: String,
    pub module: String,
    pub description: String,
    pub allowed: bool,
    pub is_override: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolePermissionsResponse {
    pub role: String,
    pub is_owner: bool,
    pub permissions: Vec<RolePermissionItem>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UpdateRolePermissionsRequest {
    pub overrides: Vec<RolePermissionOverrideInput>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RolePermissionOverrideInput {
    pub permission_code: String,
    pub allowed: bool,
}

/// Lista el catalogo completo de permisos del sistema.
pub async fn list_permissions(pool: &PgPool, claims: &Claims) -> Result<Vec<Permission>, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("users:manage")?;

    let perms = sqlx::query_as::<_, Permission>(
        "select code, module, description from permissions order by module, code",
    )
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(perms)
}

/// Obtiene los permisos efectivos de un rol para el tenant del usuario.
pub async fn get_role_permissions(
    pool: &PgPool,
    claims: &Claims,
    role_code: &str,
) -> Result<RolePermissionsResponse, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require_permission("users:manage")?;

    let resp = get_role_permissions_tx(&mut tx, scope.tenant, role_code).await?;
    tx.commit().await?;
    Ok(resp)
}

pub async fn get_role_permissions_tx(
    tx: &mut Tx<'_>,
    tenant_id: Uuid,
    role_code: &str,
) -> Result<RolePermissionsResponse, ApiError> {
    // Verificar que el rol existe en la maestra
    let role_id: Option<i32> = sqlx::query_scalar(
        "select tima_id from tbl_tipos_maestra
          where padre_id = (select tima_id from tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
            and tima_codigo = $1",
    )
    .bind(role_code)
    .fetch_optional(&mut **tx)
    .await?;

    let role_id = role_id.ok_or_else(|| ApiError::Domain(DomainError::NotFound))?;

    let all_perms = sqlx::query_as::<_, Permission>(
        "select code, module, description from permissions order by module, code",
    )
    .fetch_all(&mut **tx)
    .await?;

    if role_code == "DUENO" {
        let items = all_perms
            .into_iter()
            .map(|p| RolePermissionItem {
                code: p.code,
                module: p.module,
                description: p.description,
                allowed: true,
                is_override: false,
            })
            .collect();

        return Ok(RolePermissionsResponse {
            role: role_code.to_string(),
            is_owner: true,
            permissions: items,
        });
    }

    // Permisos base asignados al rol
    let base_allowed: HashSet<String> =
        sqlx::query_scalar("select permission_code from role_permissions where role_id = $1")
            .bind(role_id)
            .fetch_all(&mut **tx)
            .await?
            .into_iter()
            .collect();

    // Overrides guardados para este tenant
    let overrides: Vec<(String, bool)> = sqlx::query_as(
        "select permission_code, allowed from tenant_role_overrides
          where tenant_id = $1 and role_id = $2",
    )
    .bind(tenant_id)
    .bind(role_id)
    .fetch_all(&mut **tx)
    .await?;

    let override_map: std::collections::HashMap<String, bool> = overrides.into_iter().collect();

    let items = all_perms
        .into_iter()
        .map(|p| {
            let is_override = override_map.contains_key(&p.code);
            let allowed = override_map
                .get(&p.code)
                .copied()
                .unwrap_or_else(|| base_allowed.contains(&p.code));

            RolePermissionItem {
                code: p.code,
                module: p.module,
                description: p.description,
                allowed,
                is_override,
            }
        })
        .collect();

    Ok(RolePermissionsResponse {
        role: role_code.to_string(),
        is_owner: false,
        permissions: items,
    })
}

/// Actualiza los overrides de permisos para un rol en el negocio.
/// Solo DUENO puede ejecutarlo y nunca sobre el rol DUENO [P].
pub async fn update_role_permissions(
    pool: &PgPool,
    claims: &Claims,
    role_code: &str,
    req: UpdateRolePermissionsRequest,
) -> Result<RolePermissionsResponse, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;

    // Exige que sea DUENO explícitamente
    if scope.role != "DUENO" {
        return Err(ApiError::Domain(DomainError::Forbidden));
    }

    // [P] Regla: nunca sobre DUENO
    if role_code == "DUENO" {
        return Err(state("cannot modify permissions for DUENO"));
    }

    let role_id: Option<i32> = sqlx::query_scalar(
        "select tima_id from tbl_tipos_maestra
          where padre_id = (select tima_id from tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
            and tima_codigo = $1",
    )
    .bind(role_code)
    .fetch_optional(&mut *tx)
    .await?;

    let role_id = role_id.ok_or_else(|| ApiError::Domain(DomainError::NotFound))?;

    for item in req.overrides {
        sqlx::query(
            "insert into tenant_role_overrides (tenant_id, role_id, permission_code, allowed)
             values ($1, $2, $3, $4)
             on conflict (tenant_id, role_id, permission_code)
             do update set allowed = excluded.allowed, created_at = now()",
        )
        .bind(scope.tenant)
        .bind(role_id)
        .bind(&item.permission_code)
        .bind(item.allowed)
        .execute(&mut *tx)
        .await?;
    }

    let resp = get_role_permissions_tx(&mut tx, scope.tenant, role_code).await?;
    tx.commit().await?;
    Ok(resp)
}

/// Carga el conjunto de permisos efectivos de un usuario en un tenant para almacenar en Scope.
pub async fn load_effective_permissions_tx(
    tx: &mut Tx<'_>,
    tenant_id: Uuid,
    role_code: &str,
) -> Result<HashSet<String>, ApiError> {
    if role_code == "DUENO" {
        // Dueño siempre tiene todos los permisos
        let all: Vec<String> = sqlx::query_scalar("select code from permissions")
            .fetch_all(&mut **tx)
            .await?;
        return Ok(all.into_iter().collect());
    }

    let role_id: Option<i32> = sqlx::query_scalar(
        "select tima_id from tbl_tipos_maestra
          where padre_id = (select tima_id from tbl_tipos_maestra where padre_id is null and tima_codigo = 'ROL')
            and tima_codigo = $1",
    )
    .bind(role_code)
    .fetch_optional(&mut **tx)
    .await?;

    let Some(role_id) = role_id else {
        return Ok(HashSet::new());
    };

    // Permisos base
    let base: Vec<String> =
        sqlx::query_scalar("select permission_code from role_permissions where role_id = $1")
            .bind(role_id)
            .fetch_all(&mut **tx)
            .await?;

    let mut perms: HashSet<String> = base.into_iter().collect();

    // Aplicar overrides del tenant
    let overrides: Vec<(String, bool)> = sqlx::query_as(
        "select permission_code, allowed from tenant_role_overrides
          where tenant_id = $1 and role_id = $2",
    )
    .bind(tenant_id)
    .bind(role_id)
    .fetch_all(&mut **tx)
    .await?;

    for (code, allowed) in overrides {
        if allowed {
            perms.insert(code);
        } else {
            perms.remove(&code);
        }
    }

    Ok(perms)
}
