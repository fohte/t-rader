use std::collections::HashSet;

use chrono::{DateTime, Utc};
use quick_xml::Reader;
use quick_xml::events::Event;
use reqwest::Url;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder};

use crate::data_provider::DataProviderError;
use crate::data_provider::news::{NewsAggregator, NewsItem};
use crate::entities::rss_feed;

const HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
/// snippet を本文先頭から切り出す最大長 (バイトではなく文字数)
const SNIPPET_MAX_CHARS: usize = 280;

#[derive(Debug)]
pub struct RssFeed {
    pub source: String,
    pub url: String,
}

/// 公開 RSS 集約 NewsAggregator
///
/// フィード一覧は固定ではなく `rss_feed` テーブルから `fetch_news` 呼び出しごとに
/// 再読込する。UI / MCP からの追加・無効化が次の tick で反映される。
pub struct RssNewsAggregator {
    http: reqwest::Client,
    db: DatabaseConnection,
}

impl RssNewsAggregator {
    pub fn from_db(db: DatabaseConnection) -> Result<Self, DataProviderError> {
        let http = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .user_agent("t-rader/0.1 (news aggregator)")
            .build()
            .map_err(|e| DataProviderError::Network(e.to_string()))?;
        Ok(Self { http, db })
    }

    async fn load_feeds(&self) -> Result<Vec<RssFeed>, DataProviderError> {
        let rows = rss_feed::Entity::find()
            .filter(rss_feed::Column::Enabled.eq(true))
            .order_by_asc(rss_feed::Column::DisplayName)
            .all(&self.db)
            .await
            .map_err(|e| DataProviderError::Database(e.to_string()))?;
        Ok(rows
            .into_iter()
            .map(|row| RssFeed {
                source: row.display_name,
                url: row.url,
            })
            .collect())
    }

    async fn fetch_feed(&self, feed: &RssFeed) -> Result<Vec<NewsItem>, DataProviderError> {
        // フィードの URL を `Url` 経由で正規化することで、設定ミスを早期に検出する
        let url = Url::parse(&feed.url)
            .map_err(|e| DataProviderError::Parse(format!("invalid feed URL: {e}")))?;
        let response = self
            .http
            .get(url)
            .send()
            .await
            .map_err(|e| DataProviderError::Network(e.to_string()))?;
        let status = response.status().as_u16();
        if !(200..300).contains(&status) {
            return Err(DataProviderError::Api {
                status,
                message: format!("rss returned status {status}"),
            });
        }
        let body = response
            .text()
            .await
            .map_err(|e| DataProviderError::Network(e.to_string()))?;
        parse_rss(&feed.source, &body)
    }
}

#[async_trait::async_trait]
impl NewsAggregator for RssNewsAggregator {
    async fn fetch_news(&self) -> Result<Vec<NewsItem>, DataProviderError> {
        let feeds = self.load_feeds().await?;
        // 個別フィードの失敗で全体を倒さない。warn ログだけ残し、取れた分を返す
        let mut all: Vec<NewsItem> = Vec::new();
        for feed in &feeds {
            match self.fetch_feed(feed).await {
                Ok(items) => all.extend(items),
                Err(err) => {
                    tracing::warn!(source = %feed.source, %err, "rss feed fetch failed");
                }
            }
        }

        // URL で dedup。最初に出現した方を残す (フィード優先度順)
        let mut seen: HashSet<String> = HashSet::new();
        all.retain(|item| seen.insert(item.url.clone()));

        // 個別フィードの fetch/parse 失敗は warn で握り潰す方針なので、合算結果が 0 件でも
        // それは Parse エラーではない (祝日朝・全フィード一時的に空も合法な 0 件)。
        // 本物の永続的問題は個別 fetch_feed の warn ログで検知する。
        Ok(all)
    }
}

/// RSS 2.0 を最小限パースする。`<item>` の `title` / `link` / `description` / `pubDate`
/// のみ拾い、その他のタグは無視する。
///
/// Atom 1.0 (`<entry>` / `<published>` / `<summary>`) には対応していない。フィード側で
/// 形式が切り替わった場合は 0 件返り、`spawn_poll` の warn ログにのみ現れる。
/// Atom 化が判明したフィードは設定から外す運用前提。
pub fn parse_rss(source: &str, body: &str) -> Result<Vec<NewsItem>, DataProviderError> {
    let mut reader = Reader::from_str(body);
    reader.config_mut().trim_text(true);

    let mut items: Vec<NewsItem> = Vec::new();
    let mut in_item = false;
    // 興味のあるフィールドごとに on/off フラグを持つ。`<description><a>...</a></description>`
    // のようにフィールド内側に未知タグがあっても、内側の End で誤って off にならない
    // (内側タグは title/link/description/pubDate のどれにもマッチしないため)
    let mut in_title = false;
    let mut in_link = false;
    let mut in_description = false;
    let mut in_pub_date = false;
    let mut buf_title = String::new();
    let mut buf_link = String::new();
    let mut buf_description = String::new();
    let mut buf_pub_date = String::new();

    loop {
        match reader
            .read_event()
            .map_err(|e| DataProviderError::Parse(format!("xml: {e}")))?
        {
            Event::Start(e) => {
                let name = e.name().as_ref().to_vec();
                if name == b"item" {
                    in_item = true;
                    in_title = false;
                    in_link = false;
                    in_description = false;
                    in_pub_date = false;
                    buf_title.clear();
                    buf_link.clear();
                    buf_description.clear();
                    buf_pub_date.clear();
                } else if in_item {
                    match name.as_slice() {
                        b"title" => in_title = true,
                        b"link" => in_link = true,
                        b"description" => in_description = true,
                        b"pubDate" => in_pub_date = true,
                        _ => {}
                    }
                }
            }
            Event::End(e) => {
                let name = e.name().as_ref().to_vec();
                if name == b"item" {
                    in_item = false;
                    in_title = false;
                    in_link = false;
                    in_description = false;
                    in_pub_date = false;
                    if let Some(item) = build_item(
                        source,
                        buf_title.trim(),
                        buf_link.trim(),
                        buf_description.trim(),
                        buf_pub_date.trim(),
                    ) {
                        items.push(item);
                    }
                } else if in_item {
                    match name.as_slice() {
                        b"title" => in_title = false,
                        b"link" => in_link = false,
                        b"description" => in_description = false,
                        b"pubDate" => in_pub_date = false,
                        _ => {}
                    }
                }
            }
            Event::Text(t) => {
                if in_item {
                    let text = t
                        .decode()
                        .map_err(|e| DataProviderError::Parse(format!("text: {e}")))?;
                    append_text(
                        text.as_ref(),
                        in_title,
                        in_link,
                        in_description,
                        in_pub_date,
                        &mut buf_title,
                        &mut buf_link,
                        &mut buf_description,
                        &mut buf_pub_date,
                    );
                }
            }
            Event::CData(c) => {
                if in_item {
                    let text = String::from_utf8(c.into_inner().into_owned())
                        .map_err(|e| DataProviderError::Parse(format!("cdata: {e}")))?;
                    append_text(
                        &text,
                        in_title,
                        in_link,
                        in_description,
                        in_pub_date,
                        &mut buf_title,
                        &mut buf_link,
                        &mut buf_description,
                        &mut buf_pub_date,
                    );
                }
            }
            Event::Eof => break,
            _ => {}
        }
    }

    Ok(items)
}

#[expect(
    clippy::too_many_arguments,
    reason = "RSS field buffers と flag を並べる関数"
)]
fn append_text(
    text: &str,
    in_title: bool,
    in_link: bool,
    in_description: bool,
    in_pub_date: bool,
    title: &mut String,
    link: &mut String,
    description: &mut String,
    pub_date: &mut String,
) {
    if in_title {
        title.push_str(text);
    } else if in_link {
        link.push_str(text);
    } else if in_description {
        description.push_str(text);
    } else if in_pub_date {
        pub_date.push_str(text);
    }
}

fn build_item(
    source: &str,
    title: &str,
    link: &str,
    description: &str,
    pub_date: &str,
) -> Option<NewsItem> {
    if title.is_empty() || link.is_empty() {
        return None;
    }
    let published_at = parse_pub_date(pub_date)?;
    // CDATA セクション内の HTML エンティティは quick_xml の Text decode を経由しないため
    // ここで手動でデコードする。title 側も CDATA で来うる (Reuters JP 等) ので両方適用する。
    let title = decode_html_entities(title);
    // description は HTML を含むことがある (Yahoo / Bloomberg / Reuters の RSS は <p>...</p>
    // を CDATA で入れてくる)。表示にも interest substring match にも生 HTML を残したくないので
    // タグを削ってから truncate する。
    let cleaned = decode_html_entities(&strip_html_tags(description));
    let trimmed = cleaned.trim();
    let snippet = (!trimmed.is_empty()).then(|| truncate_chars(trimmed, SNIPPET_MAX_CHARS));
    Some(NewsItem {
        source: source.to_string(),
        url: link.to_string(),
        title: title.trim().to_string(),
        body_snippet: snippet,
        published_at,
    })
}

/// 最低限の HTML エンティティをデコードする (`&amp;` `&lt;` `&gt;` `&quot;` `&apos;` `&#NNN;` `&#xHHH;`)。
/// 完全な HTML エンティティ仕様は実装しない — RSS で実用上現れるのはこの範囲。
fn decode_html_entities(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut iter = s.chars().peekable();
    while let Some(c) = iter.next() {
        if c != '&' {
            out.push(c);
            continue;
        }
        let mut name = String::new();
        let mut found_semi = false;
        // 文字列終端まで `;` を見ずに抜けた場合に、それまで読み取った `&name` を消失させない
        // (`&world` `&id=123` のような non-entity 文字列を想定)
        let mut handled = false;
        for nc in iter.by_ref() {
            if nc == ';' {
                found_semi = true;
                break;
            }
            if nc.is_whitespace() || nc == '&' {
                out.push('&');
                out.push_str(&name);
                out.push(nc);
                handled = true;
                break;
            }
            name.push(nc);
            if name.len() > 8 {
                // RSS で見る named entity は最長 "&apos;" 程度。長すぎるならエンティティではない
                out.push('&');
                out.push_str(&name);
                handled = true;
                break;
            }
        }
        if handled {
            continue;
        }
        if !found_semi {
            out.push('&');
            out.push_str(&name);
            continue;
        }
        let decoded: Option<String> = match name.as_str() {
            "amp" => Some("&".into()),
            "lt" => Some("<".into()),
            "gt" => Some(">".into()),
            "quot" => Some("\"".into()),
            "apos" => Some("'".into()),
            "nbsp" => Some(" ".into()),
            other if other.starts_with("#x") || other.starts_with("#X") => {
                u32::from_str_radix(&other[2..], 16)
                    .ok()
                    .and_then(char::from_u32)
                    .map(|c| c.to_string())
            }
            other if other.starts_with('#') => other[1..]
                .parse::<u32>()
                .ok()
                .and_then(char::from_u32)
                .map(|c| c.to_string()),
            _ => None,
        };
        if let Some(d) = decoded {
            out.push_str(&d);
        } else {
            out.push('&');
            out.push_str(&name);
            out.push(';');
        }
    }
    out
}

/// `<tag>` `<tag attr="x">` `</tag>` を全て削る簡易関数。連続する空白は 1 つに圧縮する。
/// 完全な HTML パースではないが、RSS description に入る程度のタグ除去には十分。
fn strip_html_tags(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_tag = false;
    let mut last_was_space = false;
    for c in s.chars() {
        if in_tag {
            if c == '>' {
                in_tag = false;
                if !last_was_space && !out.is_empty() {
                    out.push(' ');
                    last_was_space = true;
                }
            }
            continue;
        }
        if c == '<' {
            in_tag = true;
            continue;
        }
        if c.is_whitespace() {
            if !last_was_space && !out.is_empty() {
                out.push(' ');
                last_was_space = true;
            }
        } else {
            out.push(c);
            last_was_space = false;
        }
    }
    out
}

fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        s.chars().take(max_chars).collect()
    }
}

/// RSS 2.0 の RFC 822 形式 (`Thu, 25 Jun 2026 09:00:00 +0900`) と、保険として
/// RFC 3339 をパースする。失敗したら None
fn parse_pub_date(s: &str) -> Option<DateTime<Utc>> {
    if let Ok(dt) = DateTime::parse_from_rfc2822(s) {
        return Some(dt.with_timezone(&Utc));
    }
    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.with_timezone(&Utc));
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use indoc::indoc;
    use rstest::rstest;

    fn ymd_hms(year: i32, mon: u32, day: u32, h: u32, m: u32, s: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(year, mon, day, h, m, s)
            .single()
            .expect("valid time")
    }

    #[rstest]
    fn parse_rss_returns_items() {
        let xml = indoc! {r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <rss version="2.0">
              <channel>
                <title>Sample Feed</title>
                <item>
                  <title>トヨタ自動車 通期決算発表</title>
                  <link>https://example.com/news/1</link>
                  <description>自動車セクター好調。</description>
                  <pubDate>Thu, 25 Jun 2026 09:00:00 +0900</pubDate>
                </item>
                <item>
                  <title><![CDATA[半導体 関連株が上昇]]></title>
                  <link>https://example.com/news/2</link>
                  <description><![CDATA[半導体テーマで物色。]]></description>
                  <pubDate>Thu, 25 Jun 2026 10:30:00 +0900</pubDate>
                </item>
              </channel>
            </rss>
        "#};

        assert_eq!(
            parse_rss("Test", xml).expect("parse ok"),
            vec![
                NewsItem {
                    source: "Test".into(),
                    url: "https://example.com/news/1".into(),
                    title: "トヨタ自動車 通期決算発表".into(),
                    body_snippet: Some("自動車セクター好調。".into()),
                    published_at: ymd_hms(2026, 6, 25, 0, 0, 0),
                },
                NewsItem {
                    source: "Test".into(),
                    url: "https://example.com/news/2".into(),
                    title: "半導体 関連株が上昇".into(),
                    body_snippet: Some("半導体テーマで物色。".into()),
                    published_at: ymd_hms(2026, 6, 25, 1, 30, 0),
                },
            ],
        );
    }

    #[rstest]
    fn parse_rss_keeps_text_across_nested_tags() {
        // description の内部に <b> のような未知タグが入っても、内側の End で description
        // フラグが落ちずに後続テキストを取りこぼさない
        let xml = indoc! {r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <rss version="2.0">
              <channel>
                <item>
                  <title>テスト</title>
                  <link>https://ex.com/n</link>
                  <description>前<b>中</b>後</description>
                  <pubDate>Thu, 25 Jun 2026 09:00:00 +0900</pubDate>
                </item>
              </channel>
            </rss>
        "#};

        assert_eq!(
            parse_rss("Test", xml).expect("parse ok"),
            vec![NewsItem {
                source: "Test".into(),
                url: "https://ex.com/n".into(),
                title: "テスト".into(),
                body_snippet: Some("前中後".into()),
                published_at: ymd_hms(2026, 6, 25, 0, 0, 0),
            }],
        );
    }

    #[rstest]
    fn parse_rss_skips_items_missing_required_fields() {
        let xml = indoc! {r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <rss version="2.0">
              <channel>
                <item>
                  <link>https://example.com/news/3</link>
                  <pubDate>Thu, 25 Jun 2026 09:00:00 +0900</pubDate>
                </item>
                <item>
                  <title>無効 pubDate</title>
                  <link>https://example.com/news/4</link>
                  <pubDate>not-a-date</pubDate>
                </item>
                <item>
                  <title>有効</title>
                  <link>https://example.com/news/5</link>
                  <pubDate>Thu, 25 Jun 2026 11:00:00 +0900</pubDate>
                </item>
              </channel>
            </rss>
        "#};

        assert_eq!(
            parse_rss("Test", xml).expect("parse ok"),
            vec![NewsItem {
                source: "Test".into(),
                url: "https://example.com/news/5".into(),
                title: "有効".into(),
                body_snippet: None,
                published_at: ymd_hms(2026, 6, 25, 2, 0, 0),
            }],
        );
    }

    #[rstest]
    #[case::no_tags("hello world", "hello world")]
    #[case::single_tag("<p>本文です</p>", "本文です")]
    #[case::nested("<p>外<span>内</span>側</p>", "外 内 側")]
    #[case::attribute(r#"<a href="x" class="y">link</a> tail"#, "link tail")]
    #[case::whitespace_collapse(indoc! {"
        a   b

        c"}, "a b c")]
    fn strip_html_tags_normalizes(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(strip_html_tags(input).trim(), expected);
    }

    #[rstest]
    #[case::ascii_truncated("abcde", 3, "abc")]
    #[case::multibyte_truncated("あいうえお", 3, "あいう")]
    #[case::under_max_unchanged("abc", 10, "abc")]
    fn truncate_chars_caps_by_character_count(
        #[case] input: &str,
        #[case] max: usize,
        #[case] expected: &str,
    ) {
        assert_eq!(truncate_chars(input, max), expected);
    }

    #[rstest]
    #[case::amp("AT&amp;T", "AT&T")]
    #[case::quote("&quot;hello&quot;", "\"hello\"")]
    #[case::numeric("&#39;25 通期", "'25 通期")]
    #[case::hex("&#x27;25 通期", "'25 通期")]
    #[case::unknown_kept_as_is("&unknown;", "&unknown;")]
    #[case::no_semicolon("a & b", "a & b")]
    #[case::mixed("&lt;p&gt;A&amp;B&lt;/p&gt;", "<p>A&B</p>")]
    #[case::ampersand_then_word_no_semi("&world", "&world")]
    #[case::trailing_ampersand("foo&", "foo&")]
    #[case::url_query_no_entity("path?a=1&id=123", "path?a=1&id=123")]
    fn decode_html_entities_handles_common_cases(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(decode_html_entities(input), expected);
    }
}
