use chrono::{DateTime, FixedOffset};
use serde::Serialize;
use utoipa::ToSchema;

use gateway_postgres::entities::{indicator, sector, stock, theme};

#[derive(Debug, Clone, Serialize, ToSchema)]
#[schema(as = Stock)]
pub struct StockResponse {
    pub id: String,
    pub name: String,
    pub market: Option<String>,
    pub sector_id: Option<String>,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub created_at: DateTime<FixedOffset>,
    #[schema(value_type = chrono::DateTime<chrono::Utc>)]
    pub updated_at: DateTime<FixedOffset>,
    pub product_category: Option<String>,
}

impl From<stock::Model> for StockResponse {
    fn from(model: stock::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            market: model.market,
            sector_id: model.sector_id,
            created_at: model.created_at,
            updated_at: model.updated_at,
            product_category: model.product_category,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[schema(as = Indicator)]
pub struct IndicatorResponse {
    pub id: String,
    pub name: String,
    pub kind: String,
}

impl From<indicator::Model> for IndicatorResponse {
    fn from(model: indicator::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            kind: model.kind,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[schema(as = Sector)]
pub struct SectorResponse {
    pub id: String,
    pub name: String,
}

impl From<sector::Model> for SectorResponse {
    fn from(model: sector::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
        }
    }
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[schema(as = Theme)]
pub struct ThemeResponse {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

impl From<theme::Model> for ThemeResponse {
    fn from(model: theme::Model) -> Self {
        Self {
            id: model.id,
            name: model.name,
            description: model.description,
        }
    }
}

/// `[[kind:id]]` のリンクテキストを解決した結果
#[derive(Debug, PartialEq, Eq, Serialize, ToSchema)]
pub struct RefResolution {
    /// "stock" | "indicator" | "sector" | "theme"
    pub kind: String,
    /// 別名で解決できた場合、入力ではなく正規の id
    pub id: String,
    /// 一致しなかった場合は None
    pub name: Option<String>,
}
