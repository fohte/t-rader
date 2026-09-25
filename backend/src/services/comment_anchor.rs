//! バージョンコメントの行範囲を検証する。

use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, QueryFilter};

use crate::entities::note_version;
use crate::error::AppError;

/// `body_md` 中から `anchor_text` を検索し、見つかれば 1-indexed の `(start_line, end_line)` を返す。
/// 複数箇所に一致する場合は位置を一意に決められないため `None` を返す。
pub fn locate_anchor(body_md: &str, anchor_text: &str) -> Option<(i32, i32)> {
    if anchor_text.is_empty() {
        return None;
    }
    if body_md.matches(anchor_text).count() != 1 {
        return None;
    }
    let pos = body_md.find(anchor_text)?;
    let start_line = body_md[..pos].matches('\n').count() as i32 + 1;
    let end_line = start_line + anchor_text.matches('\n').count() as i32;
    Some((start_line, end_line))
}

/// `note_version` コメントの行範囲を本文と照合して検証する。
pub async fn validate_version_anchor<C: ConnectionTrait>(
    db: &C,
    target_kind: &str,
    target_id: uuid::Uuid,
    anchor_side: Option<&str>,
    start_line: Option<i32>,
    end_line: Option<i32>,
) -> Result<(Option<i32>, Option<i32>), AppError> {
    if target_kind != "note_version" {
        if anchor_side.is_some() || start_line.is_some() || end_line.is_some() {
            return Err(AppError::Validation(
                "line anchors are only supported for note_version comments".into(),
            ));
        }
        return Ok((None, None));
    }

    let version = note_version::Entity::find_by_id(target_id)
        .one(db)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("note version {target_id} not found")))?;
    let has_line_data = start_line.is_some() || end_line.is_some();
    if anchor_side.is_none() && !has_line_data {
        return Ok((None, None));
    }

    let (Some(anchor_side), Some(start_line), Some(end_line)) = (anchor_side, start_line, end_line)
    else {
        return Err(AppError::Validation(
            "anchor_side, start_line, and end_line must be provided together".into(),
        ));
    };
    if !matches!(anchor_side, "old" | "new") {
        return Err(AppError::Validation(
            "anchor_side must be either old or new".into(),
        ));
    }
    if start_line < 1 || end_line < start_line {
        return Err(AppError::Validation(
            "line anchor must be a valid 1-indexed range".into(),
        ));
    }

    let body_md = if anchor_side == "new" {
        version.body_md
    } else {
        let Some(previous_version_no) = version.version_no.checked_sub(1) else {
            return Err(AppError::Validation(
                "the first version has no old-side lines".into(),
            ));
        };
        note_version::Entity::find()
            .filter(note_version::Column::NoteId.eq(version.note_id))
            .filter(note_version::Column::VersionNo.eq(previous_version_no))
            .one(db)
            .await?
            .ok_or_else(|| {
                AppError::NotFound(format!(
                    "previous version {}/{} not found",
                    version.note_id, previous_version_no
                ))
            })?
            .body_md
    };
    let line_count = body_md.split('\n').count() as i32;
    if end_line > line_count {
        return Err(AppError::Validation(format!(
            "line anchor ends at {end_line}, but the selected version has {line_count} lines"
        )));
    }

    Ok((Some(start_line), Some(end_line)))
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
    #[case::multiple_matches(indoc::indoc! {"
        dup
        b
        dup"}, "dup", None)]
    fn test_locate_anchor(
        #[case] body_md: &str,
        #[case] anchor_text: &str,
        #[case] expected: Option<(i32, i32)>,
    ) {
        assert_eq!(locate_anchor(body_md, anchor_text), expected);
    }
}
