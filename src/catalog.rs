use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::auth::Claims;
use crate::error::{ApiError, DomainError};
use crate::tenancy::{self, Action};

/// Item de catalogo resuelto para una sucursal.
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

const CATALOG_SQL: &str = r#"
select
  p.id as product_id,
  p.name,
  c.name as category,
  coalesce(bp.price, p.base_price) as price,
  tx.tima_codigo as tax_code,
  case
    when p.is_composed then
      (select min(floor((coalesce(si.on_hand, 0) - coalesce(si.reserved, 0)) / nullif(ri.qty, 0)))
         from recipe_items ri
         join recipes r2 on r2.id = ri.recipe_id
         left join stock si
           on si.product_id = ri.input_product_id and si.branch_id = $1
        where r2.product_id = p.id)
    else coalesce(s.on_hand, 0) - coalesce(s.reserved, 0)
  end as available
from branch_products bp
join products p on p.id = bp.product_id
left join product_categories c on c.id = bp.category_id
left join tbl_tipos_maestra tx on tx.tima_id = p.tax_type_id
left join stock s on s.product_id = p.id and s.branch_id = $1
where bp.branch_id = $1 and bp.is_available and p.active
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
    tx.commit().await?;

    Ok(rows
        .into_iter()
        .map(|r| CatalogItem {
            tax_rate: tax_rate_for(r.tax_code.as_deref()),
            product_id: r.product_id,
            name: r.name,
            category: r.category,
            price: r.price,
            tax_code: r.tax_code,
            available: r.available,
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
        assert_eq!(tax_rate_for(Some("OTRO")), None);
        assert_eq!(tax_rate_for(None), None);
    }
}
