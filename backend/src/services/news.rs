use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::data_provider::DataProviderError;
use crate::data_provider::news::{NewsAggregator, NewsItem};
use crate::entities::{
    indicator, news_item, news_strategy_link, sector, stock, strategy_interest, theme,
};

/// 戦略の interest から match 用語に展開した 1 行
#[derive(Debug, Clone, PartialEq, Eq)]
struct InterestTerm {
    strategy_id: Uuid,
    ref_kind: String,
    ref_id: String,
    /// match に使う表示用語 (id, name 両方を別行で生成して持つ)
    term: String,
}

/// 各 ref 種別の id → 表示名 マップを取りまとめたもの
#[derive(Debug, Default)]
struct RefNameLookup {
    stock: HashMap<String, String>,
    indicator: HashMap<String, String>,
    sector: HashMap<String, String>,
    theme: HashMap<String, String>,
}

/// fetch → upsert → link をワンサイクル実行する
pub async fn run_aggregation_cycle(
    db: &DatabaseConnection,
    aggregator: &dyn NewsAggregator,
) -> Result<AggregationStats, DataProviderError> {
    let fetched = aggregator.fetch_news().await?;
    let news_rows = upsert_news_items(db, &fetched).await.map_err(db_err)?;
    let lookup = load_ref_names(db).await.map_err(db_err)?;
    let interests = load_strategy_interests(db).await.map_err(db_err)?;
    let terms = expand_interest_terms(&interests, &lookup);
    let link_count = link_news_to_strategies(db, &news_rows, &terms)
        .await
        .map_err(db_err)?;
    Ok(AggregationStats {
        fetched: news_rows.len(),
        linked: link_count,
    })
}

/// poll task の 1 サイクルの結果統計
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AggregationStats {
    pub fetched: usize,
    pub linked: usize,
}

fn db_err(e: sea_orm::DbErr) -> DataProviderError {
    DataProviderError::Database(e.to_string())
}

/// `news_item` テーブルに upsert し、対象行の Model 全件 (title / body_snippet 等を含む)
/// を返す
pub async fn upsert_news_items(
    db: &DatabaseConnection,
    items: &[NewsItem],
) -> Result<Vec<news_item::Model>, sea_orm::DbErr> {
    if items.is_empty() {
        return Ok(Vec::new());
    }
    let now = Utc::now().into();
    let actives: Vec<news_item::ActiveModel> = items
        .iter()
        .map(|n| news_item::ActiveModel {
            id: Set(Uuid::new_v4()),
            source: Set(n.source.clone()),
            url: Set(n.url.clone()),
            title: Set(n.title.clone()),
            body_snippet: Set(n.body_snippet.clone()),
            published_at: Set(n.published_at.into()),
            fetched_at: Set(now),
        })
        .collect();

    // url で conflict したら title / source / published_at / body_snippet / fetched_at を更新
    news_item::Entity::insert_many(actives)
        .on_conflict(
            OnConflict::column(news_item::Column::Url)
                .update_columns([
                    news_item::Column::Title,
                    news_item::Column::Source,
                    news_item::Column::PublishedAt,
                    news_item::Column::BodySnippet,
                    news_item::Column::FetchedAt,
                ])
                .to_owned(),
        )
        .exec(db)
        .await?;

    let urls: Vec<String> = items.iter().map(|n| n.url.clone()).collect();
    let rows = news_item::Entity::find()
        .filter(news_item::Column::Url.is_in(urls))
        .all(db)
        .await?;
    Ok(rows)
}

async fn load_strategy_interests(
    db: &DatabaseConnection,
) -> Result<Vec<strategy_interest::Model>, sea_orm::DbErr> {
    strategy_interest::Entity::find().all(db).await
}

async fn load_ref_names(db: &DatabaseConnection) -> Result<RefNameLookup, sea_orm::DbErr> {
    let stocks = stock::Entity::find().all(db).await?;
    let indicators = indicator::Entity::find().all(db).await?;
    let sectors = sector::Entity::find().all(db).await?;
    let themes = theme::Entity::find().all(db).await?;
    Ok(RefNameLookup {
        stock: stocks.into_iter().map(|s| (s.id, s.name)).collect(),
        indicator: indicators.into_iter().map(|i| (i.id, i.name)).collect(),
        sector: sectors.into_iter().map(|s| (s.id, s.name)).collect(),
        theme: themes.into_iter().map(|t| (t.id, t.name)).collect(),
    })
}

/// 各 interest を id / 名前の両方で match 用語に展開する。
/// 名前を先に push することで、後段の `match_links` が name term を優先採用する
/// (id 文字列がたまたま title に含まれるケースでも、ユーザーに見える `matched_term`
/// は読める名前になる)。
fn expand_interest_terms(
    interests: &[strategy_interest::Model],
    lookup: &RefNameLookup,
) -> Vec<InterestTerm> {
    let mut out = Vec::new();
    for i in interests {
        let name = match i.ref_kind.as_str() {
            "stock" => lookup.stock.get(&i.ref_id),
            "indicator" => lookup.indicator.get(&i.ref_id),
            "sector" => lookup.sector.get(&i.ref_id),
            "theme" => lookup.theme.get(&i.ref_id),
            _ => None,
        };
        if let Some(name) = name
            && !name.is_empty()
            && name != &i.ref_id
        {
            push_unique(&mut out, &i.strategy_id, &i.ref_kind, &i.ref_id, name);
        }
        push_unique(&mut out, &i.strategy_id, &i.ref_kind, &i.ref_id, &i.ref_id);
    }
    out
}

fn push_unique(
    out: &mut Vec<InterestTerm>,
    strategy_id: &Uuid,
    ref_kind: &str,
    ref_id: &str,
    term: &str,
) {
    // 空 term / 短すぎる term (1 文字) は誤マッチが多すぎるため除外
    if term.chars().count() < 2 {
        return;
    }
    out.push(InterestTerm {
        strategy_id: *strategy_id,
        ref_kind: ref_kind.to_string(),
        ref_id: ref_id.to_string(),
        term: term.to_string(),
    });
}

/// news_item の title + body_snippet を term と substring match して link を作る
fn match_links(
    news_rows: &[news_item::Model],
    terms: &[InterestTerm],
) -> Vec<news_strategy_link::ActiveModel> {
    let now = Utc::now().into();
    let mut out: Vec<news_strategy_link::ActiveModel> = Vec::new();
    for news in news_rows {
        // (strategy_id, ref_kind, ref_id) 単位で重複登録を避ける
        let mut seen: std::collections::HashSet<(Uuid, String, String)> =
            std::collections::HashSet::new();
        let haystack = build_haystack(news);
        for term in terms {
            if !haystack.contains(&term.term) {
                continue;
            }
            let key = (term.strategy_id, term.ref_kind.clone(), term.ref_id.clone());
            if !seen.insert(key) {
                continue;
            }
            out.push(news_strategy_link::ActiveModel {
                news_id: Set(news.id),
                strategy_id: Set(term.strategy_id),
                ref_kind: Set(term.ref_kind.clone()),
                ref_id: Set(term.ref_id.clone()),
                matched_term: Set(term.term.clone()),
                created_at: Set(now),
            });
        }
    }
    out
}

fn build_haystack(news: &news_item::Model) -> String {
    let mut s = news.title.clone();
    if let Some(snippet) = &news.body_snippet {
        s.push(' ');
        s.push_str(snippet);
    }
    s
}

async fn link_news_to_strategies(
    db: &DatabaseConnection,
    news_rows: &[news_item::Model],
    terms: &[InterestTerm],
) -> Result<usize, sea_orm::DbErr> {
    let links = match_links(news_rows, terms);
    if links.is_empty() {
        return Ok(0);
    }
    let count = links.len();
    news_strategy_link::Entity::insert_many(links)
        .on_conflict(
            OnConflict::columns([
                news_strategy_link::Column::NewsId,
                news_strategy_link::Column::StrategyId,
                news_strategy_link::Column::RefKind,
                news_strategy_link::Column::RefId,
            ])
            .update_column(news_strategy_link::Column::MatchedTerm)
            .to_owned(),
        )
        .exec(db)
        .await?;
    Ok(count)
}

/// poll task を起動する。1 回目は即実行し、その後 `interval` で繰り返す
pub fn spawn_poll(
    db: DatabaseConnection,
    aggregator: Arc<dyn NewsAggregator>,
    interval: Duration,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            ticker.tick().await;
            match run_aggregation_cycle(&db, aggregator.as_ref()).await {
                Ok(stats) => {
                    tracing::debug!(
                        fetched = stats.fetched,
                        linked = stats.linked,
                        "news aggregation cycle completed",
                    );
                }
                Err(err) => {
                    tracing::warn!(%err, "news aggregation cycle failed");
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use rstest::rstest;
    use uuid::uuid;

    fn ymd_hms(year: i32, mon: u32, day: u32, h: u32, m: u32, s: u32) -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(year, mon, day, h, m, s)
            .single()
            .expect("valid time")
    }

    fn fake_news(id_byte: u8, title: &str, snippet: Option<&str>) -> news_item::Model {
        let mut bytes = [0u8; 16];
        bytes[0] = id_byte;
        news_item::Model {
            id: Uuid::from_bytes(bytes),
            source: "Test".into(),
            url: format!("https://example.com/news/{id_byte}"),
            title: title.into(),
            body_snippet: snippet.map(|s| s.into()),
            published_at: ymd_hms(2026, 6, 25, 0, 0, 0).into(),
            fetched_at: ymd_hms(2026, 6, 25, 0, 0, 0).into(),
        }
    }

    fn interest(strategy_id: Uuid, kind: &str, id: &str) -> strategy_interest::Model {
        strategy_interest::Model {
            strategy_id,
            ref_kind: kind.into(),
            ref_id: id.into(),
            role: "seed".into(),
            origin: "user".into(),
            created_at: ymd_hms(2026, 6, 25, 0, 0, 0).into(),
        }
    }

    const STRATEGY_A: Uuid = uuid!("00000000-0000-0000-0000-00000000000a");
    const STRATEGY_B: Uuid = uuid!("00000000-0000-0000-0000-00000000000b");

    fn lookup_with_stock(id: &str, name: &str) -> RefNameLookup {
        let mut l = RefNameLookup::default();
        l.stock.insert(id.into(), name.into());
        l
    }

    #[rstest]
    fn expand_terms_includes_name_then_id() {
        let interests = vec![interest(STRATEGY_A, "stock", "7203")];
        let lookup = lookup_with_stock("7203", "トヨタ自動車");
        assert_eq!(
            expand_interest_terms(&interests, &lookup),
            vec![
                InterestTerm {
                    strategy_id: STRATEGY_A,
                    ref_kind: "stock".into(),
                    ref_id: "7203".into(),
                    term: "トヨタ自動車".into(),
                },
                InterestTerm {
                    strategy_id: STRATEGY_A,
                    ref_kind: "stock".into(),
                    ref_id: "7203".into(),
                    term: "7203".into(),
                },
            ],
        );
    }

    #[rstest]
    fn match_links_creates_one_link_per_strategy_ref_pair() {
        let news = vec![fake_news(1, "トヨタ自動車 通期決算発表", Some("好調"))];
        let terms = vec![
            InterestTerm {
                strategy_id: STRATEGY_A,
                ref_kind: "stock".into(),
                ref_id: "7203".into(),
                term: "7203".into(),
            },
            InterestTerm {
                strategy_id: STRATEGY_A,
                ref_kind: "stock".into(),
                ref_id: "7203".into(),
                term: "トヨタ自動車".into(),
            },
            InterestTerm {
                strategy_id: STRATEGY_B,
                ref_kind: "stock".into(),
                ref_id: "7203".into(),
                term: "トヨタ自動車".into(),
            },
        ];
        let active = match_links(&news, &terms);
        // strategy A は (7203 文字列で false / トヨタ で true) でも 1 link、
        // strategy B も トヨタ で 1 link
        let mut keys: Vec<(Uuid, String, String, String)> = active
            .into_iter()
            .map(|a| {
                (
                    a.strategy_id.unwrap(),
                    a.ref_kind.unwrap(),
                    a.ref_id.unwrap(),
                    a.matched_term.unwrap(),
                )
            })
            .collect();
        keys.sort();
        assert_eq!(
            keys,
            vec![
                (
                    STRATEGY_A,
                    "stock".into(),
                    "7203".into(),
                    "トヨタ自動車".into(),
                ),
                (
                    STRATEGY_B,
                    "stock".into(),
                    "7203".into(),
                    "トヨタ自動車".into(),
                ),
            ],
        );
    }

    #[rstest]
    fn match_links_does_not_create_link_when_no_interest_match() {
        let news = vec![fake_news(2, "半導体テーマ上昇", Some("広く物色"))];
        let terms = vec![InterestTerm {
            strategy_id: STRATEGY_A,
            ref_kind: "stock".into(),
            ref_id: "7203".into(),
            term: "トヨタ自動車".into(),
        }];
        assert_eq!(match_links(&news, &terms).len(), 0);
    }

    #[rstest]
    fn match_links_matches_against_body_snippet() {
        let news = vec![fake_news(3, "市況まとめ", Some("半導体株が上昇"))];
        let terms = vec![InterestTerm {
            strategy_id: STRATEGY_A,
            ref_kind: "theme".into(),
            ref_id: "semiconductor".into(),
            term: "半導体".into(),
        }];
        let keys: Vec<(Uuid, String, String, String)> = match_links(&news, &terms)
            .into_iter()
            .map(|a| {
                (
                    a.strategy_id.unwrap(),
                    a.ref_kind.unwrap(),
                    a.ref_id.unwrap(),
                    a.matched_term.unwrap(),
                )
            })
            .collect();
        assert_eq!(
            keys,
            vec![(
                STRATEGY_A,
                "theme".into(),
                "semiconductor".into(),
                "半導体".into(),
            )],
        );
    }

    #[rstest]
    fn expand_terms_skips_single_character_term() {
        let interests = vec![interest(STRATEGY_A, "theme", "A")];
        let lookup = RefNameLookup::default();
        assert_eq!(expand_interest_terms(&interests, &lookup), vec![]);
    }
}
