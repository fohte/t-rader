use std::collections::HashSet;

use chrono::{DateTime, Utc};
use quick_xml::Reader;
use quick_xml::events::Event;
use reqwest::Url;

use crate::data_provider::DataProviderError;
use crate::data_provider::news::{NewsAggregator, NewsItem};

const HTTP_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(15);
/// snippet を本文先頭から切り出す最大長 (バイトではなく文字数)
const SNIPPET_MAX_CHARS: usize = 280;

/// 集約対象の RSS フィード一覧。表示用ソース名と URL のペア。
const DEFAULT_FEEDS: &[(&str, &str)] = &[
    (
        "Yahoo! Japan",
        "https://news.yahoo.co.jp/rss/topics/business.xml",
    ),
    (
        "Bloomberg JP",
        "https://feeds.bloomberg.co.jp/rss/markets.xml",
    ),
    ("Reuters JP", "https://jp.reuters.com/rssfeed/businessNews"),
];

pub struct RssFeed {
    pub source: String,
    pub url: String,
}

/// 公開 RSS 集約 NewsAggregator
pub struct RssNewsAggregator {
    http: reqwest::Client,
    feeds: Vec<RssFeed>,
}

impl RssNewsAggregator {
    pub fn new() -> Result<Self, DataProviderError> {
        let feeds = DEFAULT_FEEDS
            .iter()
            .map(|(s, u)| RssFeed {
                source: (*s).to_string(),
                url: (*u).to_string(),
            })
            .collect();
        Self::with_feeds(feeds)
    }

    pub fn with_feeds(feeds: Vec<RssFeed>) -> Result<Self, DataProviderError> {
        let http = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .user_agent("t-rader/0.1 (news aggregator)")
            .build()
            .map_err(|e| DataProviderError::Network(e.to_string()))?;
        Ok(Self { http, feeds })
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
        // 個別フィードの失敗で全体を倒さない。warn ログだけ残し、取れた分を返す
        let mut all: Vec<NewsItem> = Vec::new();
        for feed in &self.feeds {
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

        if all.is_empty() && !self.feeds.is_empty() {
            return Err(DataProviderError::Parse(
                "no items parsed from any rss feed".into(),
            ));
        }
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
    let mut current_tag: Option<Vec<u8>> = None;
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
                    buf_title.clear();
                    buf_link.clear();
                    buf_description.clear();
                    buf_pub_date.clear();
                } else if in_item {
                    current_tag = Some(name);
                }
            }
            Event::End(e) => {
                let name = e.name().as_ref().to_vec();
                if name == b"item" {
                    in_item = false;
                    current_tag = None;
                    if let Some(item) = build_item(
                        source,
                        buf_title.trim(),
                        buf_link.trim(),
                        buf_description.trim(),
                        buf_pub_date.trim(),
                    ) {
                        items.push(item);
                    }
                } else if current_tag.as_deref() == Some(name.as_slice()) {
                    current_tag = None;
                }
            }
            Event::Text(t) => {
                if let Some(tag) = current_tag.as_deref()
                    && in_item
                {
                    let text = t
                        .decode()
                        .map_err(|e| DataProviderError::Parse(format!("text: {e}")))?
                        .into_owned();
                    append_field(
                        tag,
                        &text,
                        &mut buf_title,
                        &mut buf_link,
                        &mut buf_description,
                        &mut buf_pub_date,
                    );
                }
            }
            Event::CData(c) => {
                if let Some(tag) = current_tag.as_deref()
                    && in_item
                {
                    let text = String::from_utf8(c.into_inner().into_owned())
                        .map_err(|e| DataProviderError::Parse(format!("cdata: {e}")))?;
                    append_field(
                        tag,
                        &text,
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

fn append_field(
    tag: &[u8],
    text: &str,
    title: &mut String,
    link: &mut String,
    description: &mut String,
    pub_date: &mut String,
) {
    match tag {
        b"title" => title.push_str(text),
        b"link" => link.push_str(text),
        b"description" => description.push_str(text),
        b"pubDate" => pub_date.push_str(text),
        _ => {}
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
    // description は HTML を含むことがある (Yahoo / Bloomberg / Reuters の RSS は <p>...</p>
    // を CDATA で入れてくる)。表示にも interest substring match にも生 HTML を残したくないので
    // タグを削ってから truncate する。
    let cleaned = strip_html_tags(description);
    let trimmed = cleaned.trim();
    let snippet = (!trimmed.is_empty()).then(|| truncate_chars(trimmed, SNIPPET_MAX_CHARS));
    Some(NewsItem {
        source: source.to_string(),
        url: link.to_string(),
        title: title.to_string(),
        body_snippet: snippet,
        published_at,
    })
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
    fn truncate_chars_caps_by_character_count() {
        assert_eq!(truncate_chars("abcde", 3), "abc");
        // multi-byte (Japanese)
        assert_eq!(truncate_chars("あいうえお", 3), "あいう");
        assert_eq!(truncate_chars("abc", 10), "abc");
    }
}
