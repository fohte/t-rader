use sea_orm::entity::prelude::*;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "watchlist_items")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub watchlist_id: Uuid,
    #[sea_orm(primary_key, auto_increment = false)]
    pub instrument_id: String,
    pub sort_order: i32,
    pub added_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(
        belongs_to = "super::watchlist::Entity",
        from = "Column::WatchlistId",
        to = "super::watchlist::Column::Id"
    )]
    Watchlist,
    #[sea_orm(
        belongs_to = "super::instrument::Entity",
        from = "Column::InstrumentId",
        to = "super::instrument::Column::Id"
    )]
    Instrument,
}

impl Related<super::watchlist::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Watchlist.def()
    }
}

impl Related<super::instrument::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Instrument.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
