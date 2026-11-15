use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{ApiError, DomainError};
use crate::tenancy::{self, Action};

/// Item de catalogo resuelto para una sucursal.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierBarcode {
    pub tier_id: Uuid,
    pub barcode: String,
}

#[derive(Debug, Serialize)]
pub struct CatalogItem {
    pub product_id: Uuid,
    pub name: String,
    pub category: Option<String>,
    pub price: Option<Decimal>,
    pub tax_code: Option<String>,
    pub tax_rate: Option<Decimal>,
    /// Unidades disponibles; `null` si no aplica.
    pub available: Option<Decimal>,
    pub tier_barcodes: Vec<TierBarcode>,
}

#[derive(Debug, sqlx::FromRow)]
struct CatalogRow {
    product_id: Uuid,
    name: String,
    category: Option<String>,
    price: Option<Decimal>,
    tax_code: Option<String>,
    available: Option<Decimal>,
}

/// Insumo de una receta con su cantidad.
#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct RecipeItem {
    pub input_product_id: Uuid,
    pub name: String,
    pub qty: Decimal,
}

pub(crate) const CATALOG_SQL: &str = r#"
select
  p.id as product_id,
  p.name,
  c.name as category,
  coalesce(bp.price, p.base_price) as price,
  tx.tima_codigo as tax_code,
  case
    when p.kind = 'SERVICIO' then null
    when p.is_composed then
      (select min(floor((coalesce(si.on_hand, 0) - coalesce(si.reserved, 0)) / nullif(ri.qty, 0)))
         from recipe_items ri
         join recipes r2 on r2.id = ri.recipe_id
         left join (
           select product_id, sum(on_hand) as on_hand, sum(reserved) as reserved
             from stock
            where branch_id = $1
            group by product_id
         ) si on si.product_id = ri.input_product_id
        where r2.product_id = p.id)
    else coalesce(s.on_hand, 0) - coalesce(s.reserved, 0)
  end as available
from branch_products bp
join products p on p.id = bp.product_id
left join product_categories c on c.id = bp.category_id
left join tbl_tipos_maestra tx on tx.tima_id = p.tax_type_id
left join (
  select product_id, sum(on_hand) as on_hand, sum(reserved) as reserved
    from stock
   where branch_id = $1
   group by product_id
) s on s.product_id = p.id
where bp.branch_id = $1 and bp.is_available and p.active
  and p.archived_at is null and p.usage = 'VENTA' and not p.has_variants
order by c.sort_order nulls first, p.name
"#;

const RECIPE_SQL: &str = r#"
select ri.input_product_id, ip.name, ri.qty
  from recipe_items ri
  join recipes r on r.id = ri.recipe_id
  join products ip on ip.id = ri.input_product_id
 where r.product_id = $1
 order by ip.name
"#;

/// Catalogo activo (solo surtido de la sucursal) con precio, impuesto y disponibilidad.
pub async fn catalog(
    pool: &PgPool,
    claims: &Claims,
    branch_id: Uuid,
) -> Result<Vec<CatalogItem>, ApiError> {
    let (scope, mut tx) = tenancy::begin_in_branch(pool, claims, branch_id).await?;
    scope.require(Action::Read)?;

    let rows = sqlx::query_as::<_, CatalogRow>(CATALOG_SQL)
        .bind(branch_id)
        .fetch_all(&mut *tx)
        .await?;

    let barcodes: Vec<(Uuid, Uuid, String)> = sqlx::query_as(
        "select product_id, tier_id, barcode
           from product_tier_prices
          where tenant_id = $1 and barcode is not null",
    )
    .bind(scope.tenant)
    .fetch_all(&mut *tx)
    .await?;

    tx.commit().await?;

    let mut barcode_map: std::collections::HashMap<Uuid, Vec<TierBarcode>> =
        std::collections::HashMap::new();
    for (pid, tid, bc) in barcodes {
        barcode_map.entry(pid).or_default().push(TierBarcode {
            tier_id: tid,
            barcode: bc,
        });
    }

    Ok(rows
        .into_iter()
        .map(|r| {
            let pid = r.product_id;
            let tb = barcode_map.remove(&pid).unwrap_or_default();
            CatalogItem {
                tax_rate: tax_rate_for(r.tax_code.as_deref()),
                product_id: pid,
                name: r.name,
                category: r.category,
                price: r.price,
                tax_code: r.tax_code,
                available: r.available,
                tier_barcodes: tb,
            }
        })
        .collect())
}

/// Devuelve los insumos y cantidades de la receta de un producto vendible.
pub async fn recipe(
    pool: &PgPool,
    claims: &Claims,
    product_id: Uuid,
) -> Result<Vec<RecipeItem>, ApiError> {
    let (scope, mut tx) = tenancy::begin(pool, claims).await?;
    scope.require(Action::Read)?;

    let exists: bool = sqlx::query_scalar("select exists(select 1 from products where id = $1)")
        .bind(product_id)
        .fetch_one(&mut *tx)
        .await?;
    if !exists {
        return Err(ApiError::Domain(DomainError::NotFound));
    }

    let items = sqlx::query_as::<_, RecipeItem>(RECIPE_SQL)
        .bind(product_id)
        .fetch_all(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(items)
}

use rust_decimal::RoundingStrategy;

pub const ICL_RATE_UNDER_35: Decimal = Decimal::from_parts(320, 0, 0, false, 0);
pub const ICL_RATE_OVER_35: Decimal = Decimal::from_parts(530, 0, 0, false, 0);

/// Calcula el Impuesto al Consumo de Licores nominal según Ley 1816 de 2016.
/// Tarifa por cada grado de alcohol en una botella de 750 ml:
/// - Hasta 35° de alcohol: $320 COP por grado.
/// - Más de 35° de alcohol: $530 COP por grado.
pub fn calculate_icl(degrees: Decimal, volume_ml: Decimal) -> Decimal {
    if degrees.is_zero() || volume_ml.is_zero() {
        return Decimal::ZERO;
    }
    let rate = if degrees <= Decimal::from(35) {
        ICL_RATE_UNDER_35
    } else {
        ICL_RATE_OVER_35
    };
    let factor = volume_ml / Decimal::from(750);
    (degrees * factor * rate).round_dp_with_strategy(2, RoundingStrategy::MidpointAwayFromZero)
}

/// Tasa de impuesto resuelta desde el codigo de la tabla maestra.
pub fn tax_rate_for(code: Option<&str>) -> Option<Decimal> {
    match code {
        Some("IVA19") => Some(Decimal::new(19, 2)),
        Some("IVA5") => Some(Decimal::new(5, 2)),
        Some("INC8") => Some(Decimal::new(8, 2)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tax_rate_resolves_known_codes() {
        assert_eq!(tax_rate_for(Some("IVA19")), Some(Decimal::new(19, 2)));
        assert_eq!(tax_rate_for(Some("IVA5")), Some(Decimal::new(5, 2)));
        assert_eq!(tax_rate_for(Some("INC8")), Some(Decimal::new(8, 2)));
        assert_eq!(tax_rate_for(Some("ICL")), None);
        assert_eq!(tax_rate_for(Some("OTRO")), None);
        assert_eq!(tax_rate_for(None), None);
    }

    #[test]
    fn calculate_icl_examples() {
        // Aguardiente 29° 750 ml: 29 * (750/750) * 320 = 9.280
        assert_eq!(
            calculate_icl(Decimal::from(29), Decimal::from(750)),
            Decimal::from(9280)
        );
        // Media de aguardiente 29° 375 ml: 29 * (375/750) * 320 = 4.640
        assert_eq!(
            calculate_icl(Decimal::from(29), Decimal::from(375)),
            Decimal::from(4640)
        );
        // Whisky 40° 1000 ml: 40 * (1000/750) * 530 = 28266.67
        assert_eq!(
            calculate_icl(Decimal::from(40), Decimal::from(1000)),
            Decimal::new(2826667, 2)
        );
        // Cero
        assert_eq!(
            calculate_icl(Decimal::ZERO, Decimal::from(750)),
            Decimal::ZERO
        );
    }
}
