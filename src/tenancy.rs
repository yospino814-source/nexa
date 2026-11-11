use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::auth::Claims;
use crate::db;
use crate::error::{ApiError, DomainError};

/// Contexto resuelto de una operacion: quien la ejecuta, sobre que tenant y
/// en que sucursal.
///
/// Se obtiene con [`begin`] / [`begin_in_branch`], que abren la transaccion,
/// validan la membresia y dejan fijado el contexto RLS. Las funciones de
/// dominio (`*_tx`) reciben la transaccion y el `Scope`, de modo que varias
/// operaciones se componen en una sola transaccion atomica.
#[derive(Debug, Clone)]
pub struct Scope {
    pub user: Uuid,
    pub tenant: Uuid,
    /// Sucursal fijada en RLS. `None` solo para membresias de todo el tenant
    /// que no pidieron una sucursal concreta (ven todas).
    pub branch: Option<Uuid>,
    /// Codigo del rol en la tabla maestra (p. ej. `ADMIN_CAJA`).
    pub role: String,
    /// Permisos granulares efectivos (base + overrides por tenant).
    pub permissions: std::collections::HashSet<String>,
}

/// Acciones protegidas por rol.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Ver catalogo, recetas y ordenes.
    Read,
    /// Crear ordenes, agregar lineas, confirmar y entregar.
    TakeOrder,
    /// Marcar ordenes como listas (cocina).
    Kitchen,
    /// Cobrar y cerrar ordenes; vender en mostrador.
    Charge,
    /// Abrir y cerrar turnos de caja.
    ManageCash,
    /// Cancelar ordenes ya creadas.
    CancelOrder,
    /// Reponer, ajustar y consultar el kardex.
    ManageInventory,
}

impl Action {
    /// Mapea cada Action al cÃ³digo de permiso granular correspondiente.
    pub fn permission_code(&self) -> &'static str {
        match self {
            Action::Read => "inventory:read",
            Action::TakeOrder => "pos:order",
            Action::Kitchen => "pos:kitchen",
            Action::Charge => "pos:sell",
            Action::ManageCash => "cash:movement",
            Action::CancelOrder => "pos:refund",
            Action::ManageInventory => "inventory:adjust",
        }
    }
}

/// Comprueba si el rol otorga el permiso de forma base.
pub fn base_allows(role: &str, code: &str) -> bool {
    match role {
        "DUENO" | "ADMIN_CAJA" => true,
        "CAJERO" => matches!(
            code,
            "inventory:read"
                | "pos:order"
                | "pos:sell"
                | "cash:movement"
                | "cash:open"
                | "cash:close"
                | "cash:history"
                | "pos:discount"
                | "pos:custom_item"
                | "customers:read"
                | "customers:write"
                | "credit:collect"
                | "credit:sell"
        ),
        "MESERO" => matches!(code, "inventory:read" | "pos:order" | "customers:read"),
        "COCINA" => matches!(code, "inventory:read" | "pos:kitchen"),
        "SUPERVISOR" => matches!(
            code,
            "inventory:read"
                | "pos:order"
                | "pos:sell"
                | "cash:movement"
                | "cash:open"
                | "cash:close"
                | "cash:history"
                | "pos:discount"
                | "pos:custom_item"
                | "customers:read"
                | "customers:write"
                | "credit:collect"
                | "credit:sell"
                | "credit:override"
                | "pos:refund"
                | "inventory:adjust"
                | "reports:read"
        ),
        "BODEGA" => matches!(
            code,
            "inventory:read"
                | "inventory:adjust"
                | "inventory:adjust_approve"
                | "inventory:transfer"
                | "purchases:receive"
        ),
        "REPARTIDOR" => matches!(code, "inventory:read" | "pos:sell"),
        _ => false,
    }
}

/// Matriz de permisos base. `DUENO` y `ADMIN_CAJA` pueden todo.
pub fn role_allows(role: &str, action: Action) -> bool {
    base_allows(role, action.permission_code())
}

impl Scope {
    /// Exige que el rol del usuario (con overrides) permita la accion.
    pub fn require(&self, action: Action) -> Result<(), ApiError> {
        self.require_permission(action.permission_code())
    }

    /// Comprueba si el rol u overrides otorgan un permiso por codigo.
    pub fn has_permission(&self, code: &str) -> bool {
        if self.role == "DUENO" {
            return true;
        }
        self.permissions.contains(code)
    }

    /// Exige un permiso por codigo granular.
    pub fn require_permission(&self, code: &str) -> Result<(), ApiError> {
        if self.has_permission(code) {
            Ok(())
        } else {
            Err(ApiError::Domain(DomainError::Forbidden))
        }
    }

    /// Si el usuario puede administrar recursos de otros (p. ej. cerrar el
    /// turno de otro cajero).
    pub fn is_admin(&self) -> bool {
        matches!(self.role.as_str(), "DUENO" | "ADMIN_CAJA")
    }
}

/// Helper para exigir un permiso granular `modulo:recurso:accion`.
#[allow(dead_code)]
pub fn require(scope: &Scope, code: &str) -> Result<(), ApiError> {
    scope.require_permission(code)
}

/// Abre una transaccion, resuelve la membresia del usuario y fija el contexto.
/// La sucursal es la del JWT si viene, o la de la membresia.
pub async fn begin<'a>(
    pool: &'a PgPool,
    claims: &Claims,
) -> Result<(Scope, Transaction<'a, Postgres>), ApiError> {
    begin_for(pool, claims, None, None).await
}

/// Como [`begin`] pero operando sobre una sucursal concreta: falla con 403 si
/// la membresia del usuario no la cubre o si no pertenece al tenant.
pub async fn begin_in_branch<'a>(
    pool: &'a PgPool,
    claims: &Claims,
    branch_id: Uuid,
) -> Result<(Scope, Transaction<'a, Postgres>), ApiError> {
    let (scope, mut tx) = begin_for(pool, claims, None, Some(branch_id)).await?;
    ensure_branch_tx(&mut tx, &scope, branch_id).await?;
    Ok((scope, tx))
}

/// Resolucion completa con tenant y/o sucursal explicitos.
pub async fn begin_for<'a>(
    pool: &'a PgPool,
    claims: &Claims,
    requested_tenant: Option<Uuid>,
    requested_branch: Option<Uuid>,
) -> Result<(Scope, Transaction<'a, Postgres>), ApiError> {
    // Las politicas de `memberships` solo dependen de auth.uid(), asi que la
    // membresia se lee antes de fijar tenant y sucursal, en la misma transaccion.
    let mut tx = db::begin_scope_tx(pool, claims, None, None).await?;
    let tenant = requested_tenant.or_else(|| claims.tenant_id());
    let branch = requested_branch.or_else(|| claims.branch_id());

    let (tenant, membership_branch, role) = membership_tx(&mut tx, tenant, branch).await?;
    // La sucursal efectiva nunca sale solo del JWT: si la membresia es de una
    // sucursal, RLS queda limitado a ella.
    let branch = branch.or(membership_branch);
    db::set_scope(&mut tx, tenant, branch).await?;

    let permissions =
        crate::permissions::load_effective_permissions_tx(&mut tx, tenant, &role).await?;

    Ok((
        Scope {
            user: claims.sub,
            tenant,
            branch,
            role,
            permissions,
        },
        tx,
    ))
}

/// Membresia que cubre (tenant, sucursal). Una membresia con `branch_id`
/// nulo cubre todas las sucursales del tenant; si hay una especifica de la
/// sucursal pedida, gana esa (su rol puede ser distinto).
async fn membership_tx(
    tx: &mut Transaction<'_, Postgres>,
    tenant: Option<Uuid>,
    branch: Option<Uuid>,
) -> Result<(Uuid, Option<Uuid>, String), ApiError> {
    let found = sqlx::query_as::<_, (Uuid, Option<Uuid>, String)>(
        "select m.tenant_id, m.branch_id, t.tima_codigo
           from memberships m
           join tbl_tipos_maestra t on t.tima_id = m.role_id
          where m.user_id = auth.uid()
            and m.active
            and ($1::uuid is null or m.tenant_id = $1)
            and ($2::uuid is null or m.branch_id is null or m.branch_id = $2)
          order by (m.branch_id is not distinct from $2) desc,
                   m.branch_id nulls first, m.created_at
          limit 1",
    )
    .bind(tenant)
    .bind(branch)
    .fetch_optional(&mut **tx)
    .await?;
    found.ok_or(ApiError::Domain(DomainError::Forbidden))
}

/// Sucursal accesible para el usuario, con el rol que tiene en ella.
/// Verifica que el usuario pueda hacer `action` tambien en otra sucursal del
/// mismo negocio (p. ej. el destino de un traslado), segun su membresia ahi.
pub async fn require_in_branch_tx(
    tx: &mut Transaction<'_, Postgres>,
    scope: &Scope,
    branch_id: Uuid,
    action: Action,
) -> Result<(), ApiError> {
    let other = Scope {
        branch: None,
        ..scope.clone()
    };
    ensure_branch_tx(tx, &other, branch_id).await?;
    let (_, _, role) = membership_tx(tx, Some(scope.tenant), Some(branch_id)).await?;
    if role_allows(&role, action) {
        Ok(())
    } else {
        Err(ApiError::Domain(DomainError::Forbidden))
    }
}

#[derive(Debug, serde::Serialize, sqlx::FromRow)]
pub struct BranchAccess {
    pub id: Uuid,
    pub name: String,
    pub code: Option<String>,
    pub role: String,
}

/// Sucursales del negocio a las que el usuario tiene acceso. Una membresia
/// especifica de la sucursal gana sobre la de todo el negocio.
pub async fn branches(pool: &PgPool, claims: &Claims) -> Result<Vec<BranchAccess>, ApiError> {
    // Sin sucursal pedida: la membresia de todo el negocio no restringe RLS,
    // y `branches` solo filtra por tenant.
    let (scope, mut tx) = begin_for(pool, claims, None, None).await?;
    let rows = sqlx::query_as::<_, BranchAccess>(
        "select b.id, b.name, b.code, coalesce(own.role, wide.role) as role
           from branches b
           left join lateral (
             select t.tima_codigo as role
               from memberships m join tbl_tipos_maestra t on t.tima_id = m.role_id
              where m.user_id = auth.uid() and m.active and m.tenant_id = b.tenant_id and m.branch_id = b.id
              limit 1) own on true
           left join lateral (
             select t.tima_codigo as role
               from memberships m join tbl_tipos_maestra t on t.tima_id = m.role_id
              where m.user_id = auth.uid() and m.active and m.tenant_id = b.tenant_id and m.branch_id is null
              limit 1) wide on true
          where b.tenant_id = $1 and coalesce(own.role, wide.role) is not null
          order by b.name",
    )
    .bind(scope.tenant)
    .fetch_all(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(rows)
}

/// Resuelve la pertenencia del usuario (tenant y rol) usando `memberships`
/// como fuente de verdad. El rol se devuelve como codigo de la tabla maestra.
#[cfg_attr(not(test), allow(dead_code))]
pub async fn resolve_membership(
    pool: &PgPool,
    claims: &Claims,
    requested_tenant: Option<Uuid>,
) -> Result<(Uuid, String), ApiError> {
    let (scope, tx) = begin_for(pool, claims, requested_tenant, None).await?;
    tx.commit().await?;
    Ok((scope.tenant, scope.role))
}

/// Verifica, dentro de la transaccion, que la sucursal pertenece al tenant y
/// que esta dentro del alcance del usuario.
pub async fn ensure_branch_tx(
    tx: &mut Transaction<'_, Postgres>,
    scope: &Scope,
    branch_id: Uuid,
) -> Result<(), ApiError> {
    if scope.branch.is_some_and(|b| b != branch_id) {
        return Err(ApiError::Domain(DomainError::Forbidden));
    }

    let exists: bool = sqlx::query_scalar(
        "select exists(select 1 from branches where tenant_id = $1 and id = $2)",
    )
    .bind(scope.tenant)
    .bind(branch_id)
    .fetch_one(&mut **tx)
    .await?;

    if exists {
        Ok(())
    } else {
        Err(ApiError::Domain(DomainError::Forbidden))
    }
}

/// Verifica que una sucursal pertenece al tenant indicado y al alcance del usuario.
#[cfg_attr(not(test), allow(dead_code))]
pub async fn ensure_branch_in_tenant(
    pool: &PgPool,
    claims: &Claims,
    tenant_id: Uuid,
    branch_id: Uuid,
) -> Result<(), ApiError> {
    let (scope, mut tx) = begin_for(pool, claims, Some(tenant_id), Some(branch_id)).await?;
    ensure_branch_tx(&mut tx, &scope, branch_id).await?;
    tx.commit().await?;
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct CreateBranchRequest {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow, Clone)]
pub struct BranchItem {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Deserialize)]
pub struct UpdateBranchRequest {
    pub name: String,
}

pub async fn create_branch(
    pool: &PgPool,
    claims: &Claims,
    req: CreateBranchRequest,
) -> Result<BranchItem, ApiError> {
    let (scope, mut tx) = begin(pool, claims).await?;
    if scope.role != "DUENO" {
        return Err(ApiError::Domain(DomainError::Forbidden));
    }
    crate::db::set_scope(&mut tx, scope.tenant, None).await?;

    let name = req.name.trim();
    let name_len = name.chars().count();
    if !(2..=60).contains(&name_len) {
        return Err(crate::error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "BRANCH_INVALID",
            "El nombre de la sucursal debe tener entre 2 y 60 caracteres.",
            None,
        ));
    }

    let exists: bool = sqlx::query_scalar(
        "select exists(select 1 from branches where tenant_id = $1 and lower(name) = lower($2))",
    )
    .bind(scope.tenant)
    .bind(name)
    .fetch_one(&mut *tx)
    .await?;

    if exists {
        return Err(crate::error::coded(
            StatusCode::CONFLICT,
            "BRANCH_DUPLICATED",
            "Ya existe una sucursal con ese nombre.",
            None,
        ));
    }

    let branch = sqlx::query_as::<_, BranchItem>(
        "insert into branches (tenant_id, name) values ($1, $2) returning id, name",
    )
    .bind(scope.tenant)
    .bind(name)
    .fetch_one(&mut *tx)
    .await?;

    crate::audit::record_tx(
        &mut tx,
        &scope,
        "SUCURSAL_CREADA",
        "branches",
        branch.id,
        None,
        Some(serde_json::to_value(&branch).unwrap()),
        None,
    )
    .await?;

    tx.commit().await?;
    Ok(branch)
}

pub async fn update_branch(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Uuid,
    req: UpdateBranchRequest,
) -> Result<BranchItem, ApiError> {
    let (scope, mut tx) = begin_in_branch(pool, claims, branch_id).await?;
    scope.require_permission("settings:write")?;

    let name = req.name.trim();
    let name_len = name.chars().count();
    if !(2..=60).contains(&name_len) {
        return Err(crate::error::coded(
            StatusCode::UNPROCESSABLE_ENTITY,
            "BRANCH_INVALID",
            "El nombre de la sucursal debe tener entre 2 y 60 caracteres.",
            None,
        ));
    }

    let exists: bool = sqlx::query_scalar(
        "select exists(select 1 from branches where tenant_id = $1 and id != $2 and lower(name) = lower($3))",
    )
    .bind(scope.tenant)
    .bind(branch_id)
    .bind(name)
    .fetch_one(&mut *tx)
    .await?;

    if exists {
        return Err(crate::error::coded(
            StatusCode::CONFLICT,
            "BRANCH_DUPLICATED",
            "Ya existe una sucursal con ese nombre.",
            None,
        ));
    }

    let old_name: String =
        sqlx::query_scalar("select name from branches where id = $1 and tenant_id = $2 for update")
            .bind(branch_id)
            .bind(scope.tenant)
            .fetch_one(&mut *tx)
            .await?;

    sqlx::query("update branches set name = $1 where id = $2 and tenant_id = $3")
        .bind(name)
        .bind(branch_id)
        .bind(scope.tenant)
        .execute(&mut *tx)
        .await?;

    let item = BranchItem {
        id: branch_id,
        name: name.to_string(),
    };

    crate::audit::record_tx(
        &mut tx,
        &scope,
        "SUCURSAL_ACTUALIZADA",
        "branches",
        branch_id,
        Some(serde_json::json!({ "name": old_name })),
        Some(serde_json::json!({ "name": name })),
        None,
    )
    .await?;

    tx.commit().await?;
    Ok(item)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_matrix() {
        use Action::*;
        for action in [
            Read,
            TakeOrder,
            Kitchen,
            Charge,
            ManageCash,
            CancelOrder,
            ManageInventory,
        ] {
            assert!(role_allows("DUENO", action));
            assert!(role_allows("ADMIN_CAJA", action));
            assert!(!role_allows("DESCONOCIDO", action));
        }
        assert!(role_allows("CAJERO", Charge));
        assert!(role_allows("CAJERO", ManageCash));
        assert!(!role_allows("CAJERO", ManageInventory));
        assert!(!role_allows("CAJERO", CancelOrder));
        assert!(role_allows("MESERO", TakeOrder));
        assert!(!role_allows("MESERO", Charge));
        assert!(role_allows("COCINA", Kitchen));
        assert!(!role_allows("COCINA", TakeOrder));
    }
}
