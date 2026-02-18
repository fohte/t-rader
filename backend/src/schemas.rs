//! SeaORM Entity の Model に対する utoipa ToSchema の手動実装。
//!
//! entities/ は sea-orm-cli で自動生成されるため手動編集禁止。
//! OpenAPI スキーマ定義はここで分離して管理する。

use utoipa::PartialSchema;
use utoipa::openapi::schema::{ObjectBuilder, SchemaFormat, Type};
use utoipa::openapi::{KnownFormat, RefOr, Schema};

// --- watchlists::Model ---

impl utoipa::ToSchema for crate::entities::watchlists::Model {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("Watchlist")
    }
}

impl PartialSchema for crate::entities::watchlists::Model {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .property(
                "id",
                ObjectBuilder::new()
                    .schema_type(Type::String)
                    .format(Some(SchemaFormat::KnownFormat(KnownFormat::Uuid))),
            )
            .required("id")
            .property("name", ObjectBuilder::new().schema_type(Type::String))
            .required("name")
            .property(
                "sort_order",
                ObjectBuilder::new()
                    .schema_type(Type::Integer)
                    .format(Some(SchemaFormat::KnownFormat(KnownFormat::Int32))),
            )
            .required("sort_order")
            .property(
                "created_at",
                ObjectBuilder::new()
                    .schema_type(Type::String)
                    .format(Some(SchemaFormat::KnownFormat(KnownFormat::DateTime))),
            )
            .required("created_at")
            .into()
    }
}

// --- bars::Model ---

impl utoipa::ToSchema for crate::entities::bars::Model {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("Bar")
    }
}

impl PartialSchema for crate::entities::bars::Model {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .property(
                "instrument_id",
                ObjectBuilder::new().schema_type(Type::String),
            )
            .required("instrument_id")
            .property(
                "timeframe",
                ObjectBuilder::new()
                    .schema_type(Type::String)
                    .enum_values(Some(["1d"])),
            )
            .required("timeframe")
            .property(
                "timestamp",
                ObjectBuilder::new()
                    .schema_type(Type::String)
                    .format(Some(SchemaFormat::KnownFormat(KnownFormat::DateTime))),
            )
            .required("timestamp")
            .property("open", ObjectBuilder::new().schema_type(Type::String))
            .required("open")
            .property("high", ObjectBuilder::new().schema_type(Type::String))
            .required("high")
            .property("low", ObjectBuilder::new().schema_type(Type::String))
            .required("low")
            .property("close", ObjectBuilder::new().schema_type(Type::String))
            .required("close")
            .property(
                "volume",
                ObjectBuilder::new()
                    .schema_type(Type::Integer)
                    .format(Some(SchemaFormat::KnownFormat(KnownFormat::Int64))),
            )
            .required("volume")
            .into()
    }
}

// --- watchlist_items::Model ---

impl utoipa::ToSchema for crate::entities::watchlist_items::Model {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("WatchlistItem")
    }
}

impl PartialSchema for crate::entities::watchlist_items::Model {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .property(
                "watchlist_id",
                ObjectBuilder::new()
                    .schema_type(Type::String)
                    .format(Some(SchemaFormat::KnownFormat(KnownFormat::Uuid))),
            )
            .required("watchlist_id")
            .property(
                "instrument_id",
                ObjectBuilder::new().schema_type(Type::String),
            )
            .required("instrument_id")
            .property(
                "sort_order",
                ObjectBuilder::new()
                    .schema_type(Type::Integer)
                    .format(Some(SchemaFormat::KnownFormat(KnownFormat::Int32))),
            )
            .required("sort_order")
            .property(
                "added_at",
                ObjectBuilder::new()
                    .schema_type(Type::String)
                    .format(Some(SchemaFormat::KnownFormat(KnownFormat::DateTime))),
            )
            .required("added_at")
            .into()
    }
}
