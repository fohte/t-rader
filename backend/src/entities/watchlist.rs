use sea_orm::entity::prelude::*;
use serde::Serialize;

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize)]
#[sea_orm(table_name = "watchlists")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: Uuid,
    pub name: String,
    pub sort_order: i32,
    pub created_at: DateTimeWithTimeZone,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::watchlist_item::Entity")]
    WatchlistItems,
}

impl Related<super::watchlist_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WatchlistItems.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
