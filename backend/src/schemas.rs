//! SeaORM Entity の Model に対する utoipa ToSchema の手動実装。
//!
//! entities/ は sea-orm-cli で自動生成されるため手動編集禁止。
//! OpenAPI スキーマ定義はここで分離して管理する。

use utoipa::PartialSchema;
use utoipa::ToSchema;
use utoipa::openapi::schema::{ArrayBuilder, ObjectBuilder, Ref, SchemaFormat, SchemaType, Type};
use utoipa::openapi::{KnownFormat, Object, RefOr, Schema};

use crate::services::graph::GraphDef;

/// 文字列プロパティ
fn str_prop() -> Object {
    ObjectBuilder::new().schema_type(Type::String).build()
}

/// UUID 文字列プロパティ
fn uuid_prop() -> Object {
    ObjectBuilder::new()
        .schema_type(Type::String)
        .format(Some(SchemaFormat::KnownFormat(KnownFormat::Uuid)))
        .build()
}

/// date-time 文字列プロパティ
fn datetime_prop() -> Object {
    ObjectBuilder::new()
        .schema_type(Type::String)
        .format(Some(SchemaFormat::KnownFormat(KnownFormat::DateTime)))
        .build()
}

/// 日付文字列プロパティ
fn date_prop() -> Object {
    ObjectBuilder::new()
        .schema_type(Type::String)
        .format(Some(SchemaFormat::KnownFormat(KnownFormat::Date)))
        .build()
}

/// i32 プロパティ
fn i32_prop() -> Object {
    ObjectBuilder::new()
        .schema_type(Type::Integer)
        .format(Some(SchemaFormat::KnownFormat(KnownFormat::Int32)))
        .build()
}

/// nullable な文字列プロパティ
fn nullable_str_prop() -> Object {
    ObjectBuilder::new()
        .schema_type(SchemaType::from_iter([Type::String, Type::Null]))
        .build()
}

/// nullable な UUID 文字列プロパティ
fn nullable_uuid_prop() -> Object {
    ObjectBuilder::new()
        .schema_type(SchemaType::from_iter([Type::String, Type::Null]))
        .format(Some(SchemaFormat::KnownFormat(KnownFormat::Uuid)))
        .build()
}

/// nullable な i32 プロパティ
fn nullable_i32_prop() -> Object {
    ObjectBuilder::new()
        .schema_type(SchemaType::from_iter([Type::Integer, Type::Null]))
        .format(Some(SchemaFormat::KnownFormat(KnownFormat::Int32)))
        .build()
}

/// 任意の数値 (rust_decimal::Decimal)。
/// rust_decimal の `serde-float` で JSON 数値として出力されるため number 型にする
fn decimal_prop() -> Object {
    ObjectBuilder::new().schema_type(Type::Number).build()
}

/// nullable な数値 (rust_decimal::Decimal)
fn nullable_decimal_prop() -> Object {
    ObjectBuilder::new()
        .schema_type(SchemaType::from_iter([Type::Number, Type::Null]))
        .build()
}

/// 任意の JSON 値
fn json_prop() -> Object {
    ObjectBuilder::new().schema_type(Type::Object).build()
}

/// bool プロパティ
fn bool_prop() -> Object {
    ObjectBuilder::new().schema_type(Type::Boolean).build()
}

/// nullable な date-time プロパティ
fn nullable_datetime_prop() -> Object {
    ObjectBuilder::new()
        .schema_type(SchemaType::from_iter([Type::String, Type::Null]))
        .format(Some(SchemaFormat::KnownFormat(KnownFormat::DateTime)))
        .build()
}

/// nullable な任意 JSON 値
fn nullable_json_prop() -> Object {
    ObjectBuilder::new()
        .schema_type(SchemaType::from_iter([Type::Object, Type::Null]))
        .build()
}

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
            .property("open", ObjectBuilder::new().schema_type(Type::Number))
            .required("open")
            .property("high", ObjectBuilder::new().schema_type(Type::Number))
            .required("high")
            .property("low", ObjectBuilder::new().schema_type(Type::Number))
            .required("low")
            .property("close", ObjectBuilder::new().schema_type(Type::Number))
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

// --- strategy::Model ---

impl utoipa::ToSchema for crate::entities::strategy::Model {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("Strategy")
    }
}

impl PartialSchema for crate::entities::strategy::Model {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .property("id", uuid_prop())
            .required("id")
            .property("name", str_prop())
            .required("name")
            .property("description", nullable_str_prop())
            .property("sort_order", i32_prop())
            .required("sort_order")
            .property("created_at", datetime_prop())
            .required("created_at")
            .property("updated_at", datetime_prop())
            .required("updated_at")
            .into()
    }
}

// --- stock::Model ---

impl utoipa::ToSchema for crate::entities::stock::Model {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("Stock")
    }
}

impl PartialSchema for crate::entities::stock::Model {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .property("id", str_prop())
            .required("id")
            .property("name", str_prop())
            .required("name")
            .property("market", nullable_str_prop())
            .property("sector_id", nullable_str_prop())
            .property("created_at", datetime_prop())
            .required("created_at")
            .property("updated_at", datetime_prop())
            .required("updated_at")
            .into()
    }
}

// --- indicator::Model ---

impl utoipa::ToSchema for crate::entities::indicator::Model {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("Indicator")
    }
}

impl PartialSchema for crate::entities::indicator::Model {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .property("id", str_prop())
            .required("id")
            .property("name", str_prop())
            .required("name")
            .property("kind", str_prop())
            .required("kind")
            .into()
    }
}

// --- sector::Model ---

impl utoipa::ToSchema for crate::entities::sector::Model {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("Sector")
    }
}

impl PartialSchema for crate::entities::sector::Model {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .property("id", str_prop())
            .required("id")
            .property("name", str_prop())
            .required("name")
            .into()
    }
}

// --- theme::Model ---

impl utoipa::ToSchema for crate::entities::theme::Model {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("Theme")
    }
}

impl PartialSchema for crate::entities::theme::Model {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .property("id", str_prop())
            .required("id")
            .property("name", str_prop())
            .required("name")
            .property("description", nullable_str_prop())
            .into()
    }
}

// --- note::Model ---

impl utoipa::ToSchema for crate::entities::note::Model {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("Note")
    }

    fn schemas(schemas: &mut Vec<(String, RefOr<Schema>)>) {
        schemas.push((GraphDef::name().into_owned(), GraphDef::schema()));
        <GraphDef as ToSchema>::schemas(schemas);
    }
}

impl PartialSchema for crate::entities::note::Model {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .property("id", uuid_prop())
            .required("id")
            .property("strategy_id", uuid_prop())
            .required("strategy_id")
            .property("title", str_prop())
            .required("title")
            .property("body_md", str_prop())
            .required("body_md")
            .property("frontmatter_json", json_prop())
            .required("frontmatter_json")
            .property("type_tag", nullable_str_prop())
            .property("status", str_prop())
            .required("status")
            .property("trigger", nullable_str_prop())
            .property("trigger_label", nullable_str_prop())
            .property("created_by_kind", str_prop())
            .required("created_by_kind")
            .property("created_at", datetime_prop())
            .required("created_at")
            .property("updated_at", datetime_prop())
            .required("updated_at")
            .property(
                "graphs_json",
                ArrayBuilder::new().items(Ref::from_schema_name(GraphDef::name())),
            )
            .required("graphs_json")
            .into()
    }
}

// --- annotation::Model ---

impl utoipa::ToSchema for crate::entities::annotation::Model {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("Annotation")
    }
}

impl PartialSchema for crate::entities::annotation::Model {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .property("id", uuid_prop())
            .required("id")
            .property("strategy_id", uuid_prop())
            .required("strategy_id")
            .property("target_symbol", str_prop())
            .required("target_symbol")
            .property("target_kind", str_prop())
            .required("target_kind")
            .property("timestamp", datetime_prop())
            .required("timestamp")
            .property("price", nullable_decimal_prop())
            .property("text", str_prop())
            .required("text")
            .property("status", str_prop())
            .required("status")
            .property("linked_note_id", nullable_uuid_prop())
            .property("created_by_kind", str_prop())
            .required("created_by_kind")
            .property("created_at", datetime_prop())
            .required("created_at")
            .property("updated_at", datetime_prop())
            .required("updated_at")
            .into()
    }
}

// --- comment::Model ---

impl utoipa::ToSchema for crate::entities::comment::Model {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("Comment")
    }
}

impl PartialSchema for crate::entities::comment::Model {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .property("id", uuid_prop())
            .required("id")
            .property("target_kind", str_prop())
            .required("target_kind")
            .property("target_id", uuid_prop())
            .required("target_id")
            .property("parent_id", nullable_uuid_prop())
            .property("body", str_prop())
            .required("body")
            .property("author_kind", str_prop())
            .required("author_kind")
            .property("author_label", str_prop())
            .required("author_label")
            .property("resolved", bool_prop())
            .required("resolved")
            .property("created_at", datetime_prop())
            .required("created_at")
            .property("anchor_text", nullable_str_prop())
            .property("start_line", nullable_i32_prop())
            .property("end_line", nullable_i32_prop())
            .property("drifted", bool_prop())
            .required("drifted")
            .into()
    }
}

// --- change_history::Model ---

impl utoipa::ToSchema for crate::entities::change_history::Model {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("ChangeHistory")
    }
}

impl PartialSchema for crate::entities::change_history::Model {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .property("id", uuid_prop())
            .required("id")
            .property("target_kind", str_prop())
            .required("target_kind")
            .property("target_id", uuid_prop())
            .required("target_id")
            .property("actor_kind", str_prop())
            .required("actor_kind")
            .property("actor_label", str_prop())
            .required("actor_label")
            .property("op", str_prop())
            .required("op")
            .property("diff_json", json_prop())
            .required("diff_json")
            .property("summary", nullable_str_prop())
            .property("created_at", datetime_prop())
            .required("created_at")
            .into()
    }
}

// --- trade::Model ---

impl utoipa::ToSchema for crate::entities::trade::Model {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("Trade")
    }
}

impl PartialSchema for crate::entities::trade::Model {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .property("id", uuid_prop())
            .required("id")
            .property("strategy_id", uuid_prop())
            .required("strategy_id")
            .property("symbol", str_prop())
            .required("symbol")
            .property("side", str_prop())
            .required("side")
            .property("qty", decimal_prop())
            .required("qty")
            .property("price", decimal_prop())
            .required("price")
            .property("fee", decimal_prop())
            .required("fee")
            .property("date", date_prop())
            .required("date")
            .property("source", str_prop())
            .required("source")
            .property("note", nullable_str_prop())
            .property("created_at", datetime_prop())
            .required("created_at")
            .property("updated_at", datetime_prop())
            .required("updated_at")
            .into()
    }
}

// --- strategy_interest::Model ---

impl utoipa::ToSchema for crate::entities::strategy_interest::Model {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("StrategyInterest")
    }
}

impl PartialSchema for crate::entities::strategy_interest::Model {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .property("strategy_id", uuid_prop())
            .required("strategy_id")
            .property("ref_kind", str_prop())
            .required("ref_kind")
            .property("ref_id", str_prop())
            .required("ref_id")
            .property("role", str_prop())
            .required("role")
            .property("origin", str_prop())
            .required("origin")
            .property("created_at", datetime_prop())
            .required("created_at")
            .into()
    }
}

// --- trigger::Model ---

impl utoipa::ToSchema for crate::entities::trigger::Model {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("Trigger")
    }
}

impl PartialSchema for crate::entities::trigger::Model {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .property("trigger_id", uuid_prop())
            .required("trigger_id")
            .property("strategy_id", uuid_prop())
            .required("strategy_id")
            .property("kind", str_prop())
            .required("kind")
            .property("schedule", nullable_str_prop())
            .property("hook_slug", nullable_str_prop())
            .property("event_match", nullable_json_prop())
            .property("prompt_template", str_prop())
            .required("prompt_template")
            .property("enabled", bool_prop())
            .required("enabled")
            .property("last_fired_at", nullable_datetime_prop())
            .property("created_at", datetime_prop())
            .required("created_at")
            .property("updated_at", datetime_prop())
            .required("updated_at")
            .into()
    }
}

// --- hypothesis::Model ---

impl utoipa::ToSchema for crate::entities::hypothesis::Model {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("Hypothesis")
    }
}

impl PartialSchema for crate::entities::hypothesis::Model {
    fn schema() -> RefOr<Schema> {
        let uuid_array = ArrayBuilder::new().items(uuid_prop()).build();
        ObjectBuilder::new()
            .property("hypothesis_id", uuid_prop())
            .required("hypothesis_id")
            .property("strategy_id", uuid_prop())
            .required("strategy_id")
            .property("title", str_prop())
            .required("title")
            .property("body", str_prop())
            .required("body")
            .property("status", str_prop())
            .required("status")
            .property("related_note_ids", uuid_array.clone())
            .required("related_note_ids")
            .property("related_interest_ids", uuid_array)
            .required("related_interest_ids")
            .property("created_at", datetime_prop())
            .required("created_at")
            .property("updated_at", datetime_prop())
            .required("updated_at")
            .into()
    }
}

// --- rss_feed::Model ---

impl utoipa::ToSchema for crate::entities::rss_feed::Model {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("RssFeed")
    }
}

impl PartialSchema for crate::entities::rss_feed::Model {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .property("id", uuid_prop())
            .required("id")
            .property("source", str_prop())
            .required("source")
            .property("display_name", str_prop())
            .required("display_name")
            .property("url", str_prop())
            .required("url")
            .property("enabled", bool_prop())
            .required("enabled")
            .property("created_at", datetime_prop())
            .required("created_at")
            .property("updated_at", datetime_prop())
            .required("updated_at")
            .into()
    }
}

// --- custom_indicator::Model ---

impl utoipa::ToSchema for crate::entities::custom_indicator::Model {
    fn name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("CustomIndicator")
    }
}

impl PartialSchema for crate::entities::custom_indicator::Model {
    fn schema() -> RefOr<Schema> {
        ObjectBuilder::new()
            .property("indicator_id", uuid_prop())
            .required("indicator_id")
            .property("name", str_prop())
            .required("name")
            .property("scope", str_prop())
            .required("scope")
            .property("strategy_id", nullable_uuid_prop())
            .property("code", str_prop())
            .required("code")
            .property("input_schema", json_prop())
            .required("input_schema")
            .property("output_schema", json_prop())
            .required("output_schema")
            .property("description", nullable_str_prop())
            .property("created_at", datetime_prop())
            .required("created_at")
            .property("updated_at", datetime_prop())
            .required("updated_at")
            .into()
    }
}
