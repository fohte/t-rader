//! コメントの位置情報 (アンカー) 追跡。
//!
//! ノート本文中の `anchor_text` から現在の行番号を再計算する。crit の
//! `anchor` + `start_line`/`end_line` + `drifted` 方式を踏襲する。

use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, IntoActiveModel, QueryFilter};

use crate::entities::comment;

/// `body_md` 中から `anchor_text` を検索し、見つかれば 1-indexed の `(start_line, end_line)` を返す。
pub fn locate_anchor(body_md: &str, anchor_text: &str) -> Option<(i32, i32)> {
    if anchor_text.is_empty() {
        return None;
    }
    let pos = body_md.find(anchor_text)?;
    let start_line = body_md[..pos].matches('\n').count() as i32 + 1;
    let end_line = start_line + anchor_text.matches('\n').count() as i32;
    Some((start_line, end_line))
}

/// note 本文が書き換わった際、その note に紐づくトップレベルコメント (parent_id が null かつ
/// anchor_text が Some) の位置を新しい body_md に対して再検索し、start_line/end_line/drifted を更新する。
pub async fn reanchor_note_comments(
    db: &sea_orm::DatabaseConnection,
    note_id: uuid::Uuid,
    body_md: &str,
) -> Result<(), sea_orm::DbErr> {
    let targets = comment::Entity::find()
        .filter(comment::Column::TargetKind.eq("note"))
        .filter(comment::Column::TargetId.eq(note_id))
        .filter(comment::Column::ParentId.is_null())
        .filter(comment::Column::AnchorText.is_not_null())
        .all(db)
        .await?;

    for c in targets {
        // 直前の filter で anchor_text が Some であることを保証済み
        let anchor_text = c.anchor_text.clone().unwrap_or_default();
        let (start_line, end_line, drifted) = match locate_anchor(body_md, &anchor_text) {
            Some((start, end)) => (Some(start), Some(end), false),
            None => (None, None, true),
        };
        let mut active = c.into_active_model();
        active.start_line = Set(start_line);
        active.end_line = Set(end_line);
        active.drifted = Set(drifted);
        active.update(db).await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::found_single_line(indoc::indoc! {"
        line1
        line2
        line3"}, "line2", Some((2, 2)))]
    #[case::found_multi_line(indoc::indoc! {"
        a
        b
        c
        d"}, "b\nc", Some((2, 3)))]
    #[case::not_found(indoc::indoc! {"
        a
        b
        c"}, "missing", None)]
    #[case::empty_anchor(indoc::indoc! {"
        a
        b
        c"}, "", None)]
    fn test_locate_anchor(
        #[case] body_md: &str,
        #[case] anchor_text: &str,
        #[case] expected: Option<(i32, i32)>,
    ) {
        assert_eq!(locate_anchor(body_md, anchor_text), expected);
    }
}
