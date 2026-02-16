use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "instruments")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = false)]
    pub id: String,
    pub name: String,
    pub market: String,
    pub sector: Option<String>,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    #[sea_orm(has_many = "super::watchlist_item::Entity")]
    WatchlistItems,
    #[sea_orm(has_many = "super::bar::Entity")]
    Bars,
}

impl Related<super::watchlist_item::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::WatchlistItems.def()
    }
}

impl Related<super::bar::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Bars.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
