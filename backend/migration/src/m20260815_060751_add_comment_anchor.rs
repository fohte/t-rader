use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::entity::prelude::Uuid;
use sea_orm_migration::sea_orm::{DbBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[derive(DeriveIden)]
enum Comment {
    Table,
    AnchorText,
    StartLine,
    EndLine,
    Drifted,
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Comment::Table)
                    .add_column_if_not_exists(ColumnDef::new(Comment::AnchorText).text())
                    .add_column_if_not_exists(ColumnDef::new(Comment::StartLine).integer())
                    .add_column_if_not_exists(ColumnDef::new(Comment::EndLine).integer())
                    .add_column_if_not_exists(
                        ColumnDef::new(Comment::Drifted)
                            .boolean()
                            .not_null()
                            .default(false),
                    )
                    .to_owned(),
            )
            .await?;

        backfill(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(Comment::Table)
                    .drop_column(Comment::AnchorText)
                    .drop_column(Comment::StartLine)
                    .drop_column(Comment::EndLine)
                    .drop_column(Comment::Drifted)
                    .to_owned(),
            )
            .await
    }
}

/// 既存コメントの `"> "` 引用プレフィックスを構造化カラムへ移す。
///
/// 対象は `body` が `"> "` で始まる行、すなわち旧フロントエンドが埋め込んだ引用付きコメントのみ。
async fn backfill(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let db = manager.get_connection();

    let rows = db
        .query_all_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT id, target_kind, target_id, body FROM comment WHERE body LIKE '> %'",
        ))
        .await?;

    for row in rows {
        let id: Uuid = row.try_get("", "id")?;
        let target_kind: String = row.try_get("", "target_kind")?;
        let target_id: Uuid = row.try_get("", "target_id")?;
        let body: String = row.try_get("", "body")?;

        let Some((anchor_text, new_body)) = extract_quote(&body) else {
            continue;
        };

        let (start_line, end_line, drifted) = if target_kind == "note" {
            let note_row = db
                .query_one_raw(Statement::from_sql_and_values(
                    DbBackend::Postgres,
                    "SELECT body_md FROM note WHERE id = $1",
                    [target_id.into()],
                ))
                .await?;
            match note_row {
                Some(note_row) => {
                    let body_md: String = note_row.try_get("", "body_md")?;
                    match locate_anchor(&body_md, &anchor_text) {
                        Some((start, end)) => (Some(start), Some(end), false),
                        None => (None, None, true),
                    }
                }
                None => (None, None, true),
            }
        } else {
            // annotation は行番号の概念が無いため、drift も起こり得ない
            (None, None, false)
        };

        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE comment SET body = $1, anchor_text = $2, start_line = $3, end_line = $4, drifted = $5 WHERE id = $6",
            [
                new_body.into(),
                anchor_text.into(),
                start_line.into(),
                end_line.into(),
                drifted.into(),
                id.into(),
            ],
        ))
        .await?;
    }

    Ok(())
}

/// `body` 先頭の連続する `"> "` 引用行を抽出し、`(anchor_text, 残りの body)` を返す。
/// 1 行目が `"> "` で始まらなければ `None`。
fn extract_quote(body: &str) -> Option<(String, String)> {
    let lines: Vec<&str> = body.split('\n').collect();
    if !lines.first()?.starts_with("> ") {
        return None;
    }
    let mut i = 0;
    let mut quote_lines = Vec::new();
    while i < lines.len() {
        let Some(rest) = lines[i].strip_prefix("> ") else {
            break;
        };
        quote_lines.push(rest);
        i += 1;
    }
    while i < lines.len() && lines[i].trim().is_empty() {
        i += 1;
    }
    Some((quote_lines.join("\n"), lines[i..].join("\n")))
}

/// `body_md` 中から `anchor_text` を検索し、見つかれば 1-indexed の `(start_line, end_line)` を返す。
fn locate_anchor(body_md: &str, anchor_text: &str) -> Option<(i32, i32)> {
    if anchor_text.is_empty() {
        return None;
    }
    let pos = body_md.find(anchor_text)?;
    let start_line = body_md[..pos].matches('\n').count() as i32 + 1;
    let end_line = start_line + anchor_text.matches('\n').count() as i32;
    Some((start_line, end_line))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_quote_extracts_leading_quote_lines() {
        assert_eq!(
            extract_quote(indoc::indoc! {"
                > foo
                > bar

                body text"}),
            Some(("foo\nbar".to_string(), "body text".to_string())),
        );
    }

    #[test]
    fn extract_quote_returns_none_without_leading_quote() {
        assert_eq!(extract_quote("no quote here"), None);
    }

    #[test]
    fn extract_quote_handles_quote_only_body() {
        assert_eq!(
            extract_quote("> only quote"),
            Some(("only quote".to_string(), String::new())),
        );
    }

    #[test]
    fn locate_anchor_finds_single_line() {
        assert_eq!(
            locate_anchor(
                indoc::indoc! {"
                line1
                line2
                line3"},
                "line2"
            ),
            Some((2, 2))
        );
    }

    #[test]
    fn locate_anchor_finds_multi_line() {
        assert_eq!(
            locate_anchor(
                indoc::indoc! {"
                a
                b
                c
                d"},
                "b\nc"
            ),
            Some((2, 3)),
        );
    }

    #[test]
    fn locate_anchor_returns_none_when_not_found() {
        assert_eq!(
            locate_anchor(
                indoc::indoc! {"
                a
                b
                c"},
                "missing"
            ),
            None
        );
    }

    #[test]
    fn locate_anchor_returns_none_for_empty_anchor() {
        assert_eq!(
            locate_anchor(
                indoc::indoc! {"
                a
                b
                c"},
                ""
            ),
            None
        );
    }
}
