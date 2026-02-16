use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "bars")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub instrument_id: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub timeframe: String,
    #[sea_orm(primary_key, auto_increment = false)]
    pub timestamp: DateTimeWithTimeZone,
    #[sea_orm(column_type = "Decimal(None)")]
    pub open: Decimal,
    #[sea_orm(column_type = "Decimal(None)")]
    pub high: Decimal,
    #[sea_orm(column_type = "Decimal(None)")]
    pub low: Decimal,
    #[sea_orm(column_type = "Decimal(None)")]
    pub close: Decimal,
    pub volume: i64,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::instrument::Entity",
        from = "Column::InstrumentId",
        to = "super::instrument::Column::Id"
    )]
    Instrument,
}

impl Related<super::instrument::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Instrument.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
