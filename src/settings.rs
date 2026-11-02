//! Ajustes por sucursal con valores por defecto combinados (F0.4).

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{ApiError, validation};
use crate::tenancy;

type Tx<'a> = Transaction<'a, Postgres>;

fn default_true() -> bool {
    true
}

fn default_false() -> bool {
    false
}

fn default_max_discount() -> Decimal {
    Decimal::new(15, 0)
}

fn default_zero() -> Decimal {
    Decimal::ZERO
}

fn default_low_stock() -> i32 {
    5
}

fn default_suggested_tips() -> Decimal {
    Decimal::new(10, 0)
}

fn default_ticket_width() -> i32 {
    80
}

fn default_costing() -> String {
    "PROMEDIO".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PosSettings {
    #[serde(default = "default_true")]
    pub allow_anonymous_sales: bool,
    #[serde(default = "default_true")]
    pub allow_alias_sales: bool,
    #[serde(default = "default_max_discount")]
    pub max_discount_pct: Decimal,
    #[serde(default = "default_false")]
    pub allow_price_override: bool,
    #[serde(default = "default_true")]
    pub refund_requires_approval: bool,
    #[serde(default = "default_true")]
    pub auto_print: bool,
}

impl Default for PosSettings {
    fn default() -> Self {
        Self {
            allow_anonymous_sales: true,
            allow_alias_sales: true,
            max_discount_pct: Decimal::new(15, 0),
            allow_price_override: false,
            refund_requires_approval: true,
            auto_print: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CashSettings {
    #[serde(default = "default_true")]
    pub require_session: bool,
    #[serde(default = "default_true")]
    pub blind_close: bool,
    #[serde(default = "default_zero")]
    pub close_tolerance: Decimal,
}

impl Default for CashSettings {
    fn default() -> Self {
        Self {
            require_session: true,
            blind_close: true,
            close_tolerance: Decimal::ZERO,
        }
    }
}

fn default_adjust_threshold() -> Decimal {
    Decimal::from(200000)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventorySettings {
    #[serde(default = "default_false")]
    pub allow_negative_stock: bool,
    #[serde(default = "default_low_stock")]
    pub low_stock_default: i32,
    #[serde(default = "default_costing")]
    pub costing: String,
    #[serde(default = "default_adjust_threshold")]
    pub adjust_approval_threshold: Decimal,
}

impl Default for InventorySettings {
    fn default() -> Self {
        Self {
            allow_negative_stock: false,
            low_stock_default: 5,
            costing: "PROMEDIO".to_string(),
            adjust_approval_threshold: Decimal::from(200000),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TipsSettings {
    #[serde(default = "default_false")]
    pub enabled: bool,
    #[serde(default = "default_suggested_tips")]
    pub suggested_pct: Decimal,
}

impl Default for TipsSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            suggested_pct: Decimal::new(10, 0),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrintSettings {
    #[serde(default = "default_ticket_width")]
    pub ticket_width_mm: i32,
}

impl Default for PrintSettings {
    fn default() -> Self {
        Self {
            ticket_width_mm: 80,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BranchSettings {
    #[serde(default)]
    pub pos: PosSettings,
    #[serde(default)]
    pub cash: CashSettings,
    #[serde(default)]
    pub inventory: InventorySettings,
    #[serde(default)]
    pub tips: TipsSettings,
    #[serde(default)]
    pub print: PrintSettings,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSettingsRequest {
    pub branch_id: Uuid,
    pub settings: BranchSettings,
}

/// Carga los ajustes de la sucursal mezclando valores de BD con los valores por defecto.
pub async fn load_tx(tx: &mut Tx<'_>, branch_id: Uuid) -> Result<BranchSettings, ApiError> {
    let row: Option<serde_json::Value> =
        sqlx::query_scalar("select data from branch_settings where branch_id = $1")
            .bind(branch_id)
            .fetch_optional(&mut **tx)
            .await?;

    match row {
        Some(val) => {
            let parsed: BranchSettings = serde_json::from_value(val).unwrap_or_default();
            Ok(parsed)
        }
        None => Ok(BranchSettings::default()),
    }
}

/// Obtiene los ajustes de una sucursal vía API.
pub async fn get_settings(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Uuid,
) -> Result<BranchSettings, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, branch_id).await?;
    scope.require_permission("inventory:read")?;

    let s = load_tx(&mut tx, branch_id).await?;
    tx.commit().await?;
    Ok(s)
}

/// Guarda ajustes de una sucursal validando rangos y tipos.
pub async fn update_settings(
    pool: &PgPool,
    claims: &Claims,
    req: UpdateSettingsRequest,
) -> Result<BranchSettings, ApiError> {
    // Validaciones
    if req.settings.pos.max_discount_pct < Decimal::ZERO
        || req.settings.pos.max_discount_pct > Decimal::new(100, 0)
    {
        return Err(validation("pos.max_discount_pct must be between 0 and 100"));
    }
    if req.settings.cash.close_tolerance < Decimal::ZERO {
        return Err(validation("cash.close_tolerance must be >= 0"));
    }
    if req.settings.inventory.low_stock_default < 0 {
        return Err(validation("inventory.low_stock_default must be >= 0"));
    }
    if req.settings.inventory.costing != "PROMEDIO" && req.settings.inventory.costing != "PEPS" {
        return Err(validation("inventory.costing must be PROMEDIO or PEPS"));
    }
    if req.settings.inventory.adjust_approval_threshold < Decimal::ZERO {
        return Err(validation(
            "inventory.adjust_approval_threshold must be >= 0",
        ));
    }
    if req.settings.tips.suggested_pct < Decimal::ZERO
        || req.settings.tips.suggested_pct > Decimal::new(100, 0)
    {
        return Err(validation("tips.suggested_pct must be between 0 and 100"));
    }
    if req.settings.print.ticket_width_mm != 80 && req.settings.print.ticket_width_mm != 58 {
        return Err(validation("print.ticket_width_mm must be 80 or 58"));
    }

    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, req.branch_id).await?;
    scope.require_permission("settings:write")?;

    let json_data = serde_json::to_value(&req.settings).map_err(anyhow::Error::from)?;

    let prev: Option<serde_json::Value> =
        sqlx::query_scalar("select data from branch_settings where branch_id = $1")
            .bind(req.branch_id)
            .fetch_optional(&mut *tx)
            .await?;

    sqlx::query(
        "insert into branch_settings (branch_id, tenant_id, data, updated_by, updated_at)
         values ($1, $2, $3, $4, now())
         on conflict (branch_id)
         do update set data = excluded.data, updated_by = excluded.updated_by, updated_at = now()",
    )
    .bind(req.branch_id)
    .bind(scope.tenant)
    .bind(&json_data)
    .bind(scope.user)
    .execute(&mut *tx)
    .await?;

    // Registrar en auditoría
    crate::audit::record_tx(
        &mut tx,
        &scope,
        "CONFIGURAR_AJUSTES",
        "branch_settings",
        req.branch_id,
        prev,
        Some(json_data),
        None,
    )
    .await?;

    tx.commit().await?;
    Ok(req.settings)
}

/// Datos del negocio para la tirilla (F1.15).
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct BusinessInfo {
    pub name: String,
    pub business_name: Option<String>,
    pub nit: Option<String>,
    pub dv: Option<String>,
}

/// `GET /v1/business`: cualquier miembro del negocio.
pub async fn business_info(pool: &PgPool, claims: &Claims) -> Result<BusinessInfo, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    let info = sqlx::query_as::<_, BusinessInfo>(
        "select name, business_name, nit, dv::text as dv from tenants where id = $1",
    )
    .bind(scope.tenant)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(info)
}
