use chrono::{DateTime, FixedOffset};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefKind {
    Stock,
    Indicator,
    Sector,
    Theme,
}

impl RefKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stock => "stock",
            Self::Indicator => "indicator",
            Self::Sector => "sector",
            Self::Theme => "theme",
        }
    }
}

impl TryFrom<&str> for RefKind {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "stock" => Ok(Self::Stock),
            "indicator" => Ok(Self::Indicator),
            "sector" => Ok(Self::Sector),
            "theme" => Ok(Self::Theme),
            _ => Err(value.to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StockRef {
    pub id: String,
    pub name: String,
    pub market: Option<String>,
    pub sector_id: Option<String>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub product_category: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndicatorRef {
    pub id: String,
    pub name: String,
    pub kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SectorRef {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeRef {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefTerm {
    pub ref_id: String,
    pub term: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefSearchMatch {
    pub ref_kind: String,
    pub ref_id: String,
    pub name: String,
    pub product_category: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedRef {
    pub kind: String,
    pub id: String,
    pub name: Option<String>,
}
