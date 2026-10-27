use std::collections::HashMap;

use crate::testing::{create_test_server_with_db, find_current_note_version, insert_test_note};
use gateway_postgres::DatabaseHandle;
use gateway_postgres::entities::{note, note_kind, note_version};
use sea_orm::ActiveModelTrait;
use sea_orm::ActiveValue::{NotSet, Set};
use serde_json::{Value, json};
use uuid::Uuid;

#[derive(Default)]
struct IdLabels {
    notes: HashMap<Uuid, String>,
    versions: HashMap<Uuid, String>,
}

struct VersionSeed<'a> {
    version_no: i32,
    status: &'a str,
    is_current: bool,
    tag: &'a str,
}

async fn insert_note_kind(db: &DatabaseHandle, key: &str, requires_approval: bool) {
    note_kind::ActiveModel {
        key: Set(key.into()),
        display_name: Set(key.into()),
        requires_approval: Set(requires_approval),
        description: Set(None),
        sort_order: Set(0),
    }
    .insert(db)
    .await
    .expect("insert note kind");
}

async fn seed_note(
    db: &DatabaseHandle,
    labels: &mut IdLabels,
    label: &str,
    kind: &str,
    versions: &[VersionSeed<'_>],
) {
    let note_id = insert_test_note(db, label, "body").await;
    note::ActiveModel {
        id: Set(note_id),
        kind: Set(Some(kind.into())),
        ..Default::default()
    }
    .update(db)
    .await
    .expect("set note kind");
    labels.notes.insert(note_id, label.into());

    let initial = find_current_note_version(db, note_id)
        .await
        .expect("find initial version")
        .expect("initial version exists");
    for seed in versions {
        let title = format!("{label}-v{}", seed.version_no);
        let frontmatter_json = json!({ "tags": [seed.tag] });
        let version_id = if seed.version_no == 1 {
            note_version::ActiveModel {
                id: Set(initial.id),
                title: Set(title),
                frontmatter_json: Set(frontmatter_json),
                status: Set(seed.status.into()),
                is_current: Set(seed.is_current),
                ..Default::default()
            }
            .update(db)
            .await
            .expect("update initial version")
            .id
        } else {
            let id = Uuid::new_v4();
            note_version::ActiveModel {
                id: Set(id),
                note_id: Set(note_id),
                version_no: Set(seed.version_no),
                title: Set(title),
                body_md: Set("body".into()),
                frontmatter_json: Set(frontmatter_json),
                graphs_json: Set(json!([])),
                resolved_price_references_json: Set(json!({})),
                warning_dispositions_json: NotSet,
                status: Set(seed.status.into()),
                is_current: Set(seed.is_current),
                change_reason: Set(None),
                created_by_kind: Set("llm".into()),
                execution_id: Set(None),
                created_at: NotSet,
                reviewed_at: Set(None),
            }
            .insert(db)
            .await
            .expect("insert note version")
            .id
        };
        labels
            .versions
            .insert(version_id, format!("{label}-v{}", seed.version_no));
    }
}

fn expected_version(
    note_label: &str,
    version_no: i32,
    status: &str,
    is_current: bool,
    tag: &str,
) -> Value {
    let version_label = format!("{note_label}-v{version_no}");
    json!({
        "id": version_label,
        "note_id": note_label,
        "version_no": version_no,
        "title": version_label,
        "body_md": "body",
        "frontmatter_json": { "tags": [tag] },
        "resolved_price_references_json": {},
        "graphs_json": [],
        "status": status,
        "is_current": is_current,
        "change_reason": null,
        "created_by_kind": if version_no == 1 { "human" } else { "llm" },
        "execution_id": null,
        "created_at": "<created_at>",
        "reviewed_at": null,
    })
}

type VersionExpectation<'a> = (i32, &'a str, bool, &'a str);

fn expected_overview(
    note_label: &str,
    kind: &str,
    representative: VersionExpectation<'_>,
    current: Option<VersionExpectation<'_>>,
    latest_unread: Option<VersionExpectation<'_>>,
    unread_version_count: i64,
) -> Value {
    let (
        representative_version_no,
        representative_status,
        representative_is_current,
        representative_tag,
    ) = representative;
    let representative_label = format!("{note_label}-v{representative_version_no}");
    let current = current.map(|(number, status, is_current, tag)| {
        expected_version(note_label, number, status, is_current, tag)
    });
    let latest_unread = latest_unread.map(|(number, status, is_current, tag)| {
        expected_version(note_label, number, status, is_current, tag)
    });
    json!({
        "representative": {
            "id": note_label,
            "version_id": representative_label,
            "version_no": representative_version_no,
            "is_current": representative_is_current,
            "title": representative_label,
            "body_md": "body",
            "frontmatter_json": { "tags": [representative_tag] },
            "resolved_price_references_json": {},
            "tags": [representative_tag],
            "kind": kind,
            "status": representative_status,
            "trigger": null,
            "trigger_label": null,
            "created_by_kind": "human",
            "created_at": "<created_at>",
            "updated_at": "<updated_at>",
            "graphs_json": [],
            "execution_id": null,
        },
        "current": current,
        "latest_unread": latest_unread,
        "unread_version_count": unread_version_count,
    })
}

fn normalize_note_overviews(mut value: Value, labels: &IdLabels) -> Value {
    let rows = value.as_array_mut().expect("overview response is an array");
    for row in rows.iter_mut() {
        let representative = row.get_mut("representative").expect("representative field");
        normalize_id(representative, "id", &labels.notes);
        normalize_id(representative, "version_id", &labels.versions);
        normalize_timestamp(representative, "created_at");
        normalize_timestamp(representative, "updated_at");
        for field in ["current", "latest_unread"] {
            let version = &mut row[field];
            if version.is_object() {
                normalize_id(version, "id", &labels.versions);
                normalize_id(version, "note_id", &labels.notes);
                normalize_timestamp(version, "created_at");
            }
        }
    }
    rows.sort_by(|left, right| {
        left["representative"]["title"]
            .as_str()
            .cmp(&right["representative"]["title"].as_str())
    });
    value
}

fn normalize_id(value: &mut Value, field: &str, labels: &HashMap<Uuid, String>) {
    let Some(id) = value
        .get(field)
        .and_then(Value::as_str)
        .and_then(|id| Uuid::parse_str(id).ok())
    else {
        return;
    };
    value[field] = json!(labels.get(&id).expect("seeded ID label"));
}

fn normalize_timestamp(value: &mut Value, field: &str) {
    if value.get(field).is_some_and(|field| !field.is_null()) {
        value[field] = json!(format!("<{field}>"));
    }
}

async fn get_json(server: &axum_test::TestServer, path: &str) -> (axum::http::StatusCode, Value) {
    let response = server.get(path).await;
    (response.status_code(), response.json())
}

#[backend_test_macros::database_test]
async fn list_note_overviews_separates_representative_versions_and_filters_by_them(
    db: DatabaseHandle,
) {
    let core_kind = "demo-core";
    let other_kind = "demo-other";
    insert_note_kind(&db, core_kind, true).await;
    insert_note_kind(&db, other_kind, false).await;

    let mut labels = IdLabels::default();
    seed_note(
        &db,
        &mut labels,
        "current-only",
        core_kind,
        &[VersionSeed {
            version_no: 1,
            status: "approved",
            is_current: true,
            tag: "current",
        }],
    )
    .await;
    seed_note(
        &db,
        &mut labels,
        "current-plus-unread",
        core_kind,
        &[
            VersionSeed {
                version_no: 1,
                status: "approved",
                is_current: true,
                tag: "current",
            },
            VersionSeed {
                version_no: 2,
                status: "unread",
                is_current: false,
                tag: "pending",
            },
        ],
    )
    .await;
    seed_note(
        &db,
        &mut labels,
        "current-unread",
        core_kind,
        &[VersionSeed {
            version_no: 1,
            status: "unread",
            is_current: true,
            tag: "current-unread",
        }],
    )
    .await;
    seed_note(
        &db,
        &mut labels,
        "multiple-unread",
        core_kind,
        &[
            VersionSeed {
                version_no: 1,
                status: "approved",
                is_current: true,
                tag: "current",
            },
            VersionSeed {
                version_no: 2,
                status: "unread",
                is_current: false,
                tag: "pending-old",
            },
            VersionSeed {
                version_no: 3,
                status: "unread",
                is_current: false,
                tag: "pending-latest",
            },
        ],
    )
    .await;
    seed_note(
        &db,
        &mut labels,
        "new-unread",
        core_kind,
        &[VersionSeed {
            version_no: 1,
            status: "unread",
            is_current: false,
            tag: "new",
        }],
    )
    .await;
    seed_note(
        &db,
        &mut labels,
        "no-current-rejected",
        core_kind,
        &[
            VersionSeed {
                version_no: 1,
                status: "approved",
                is_current: false,
                tag: "old",
            },
            VersionSeed {
                version_no: 2,
                status: "rejected",
                is_current: false,
                tag: "rejected",
            },
        ],
    )
    .await;
    seed_note(
        &db,
        &mut labels,
        "no-current-superseded",
        core_kind,
        &[
            VersionSeed {
                version_no: 1,
                status: "approved",
                is_current: false,
                tag: "old",
            },
            VersionSeed {
                version_no: 2,
                status: "superseded",
                is_current: false,
                tag: "superseded",
            },
        ],
    )
    .await;
    seed_note(
        &db,
        &mut labels,
        "version-tag",
        core_kind,
        &[
            VersionSeed {
                version_no: 1,
                status: "approved",
                is_current: false,
                tag: "historical-tag",
            },
            VersionSeed {
                version_no: 2,
                status: "approved",
                is_current: true,
                tag: "current-tag",
            },
            VersionSeed {
                version_no: 3,
                status: "unread",
                is_current: false,
                tag: "pending-tag",
            },
        ],
    )
    .await;
    seed_note(
        &db,
        &mut labels,
        "other-kind-current",
        other_kind,
        &[VersionSeed {
            version_no: 1,
            status: "approved",
            is_current: true,
            tag: "current",
        }],
    )
    .await;

    let (_db, server) = create_test_server_with_db(db).await;
    let all = get_json(&server, "/api/notes/overview").await;
    let pending = get_json(&server, "/api/notes/overview?view=pending").await;
    let by_kind = get_json(&server, "/api/notes/overview?kind=demo-core").await;
    let by_old_tag = get_json(&server, "/api/notes/overview?tag=historical-tag").await;
    let by_current_tag = get_json(&server, "/api/notes/overview?tag=current-tag").await;
    let pending_by_current_tag =
        get_json(&server, "/api/notes/overview?view=pending&tag=current-tag").await;
    let pending_by_latest_tag =
        get_json(&server, "/api/notes/overview?view=pending&tag=pending-tag").await;
    let invalid_view = get_json(&server, "/api/notes/overview?view=unknown").await;
    let actual = vec![
        (all.0, normalize_note_overviews(all.1, &labels)),
        (pending.0, normalize_note_overviews(pending.1, &labels)),
        (by_kind.0, normalize_note_overviews(by_kind.1, &labels)),
        (
            by_old_tag.0,
            normalize_note_overviews(by_old_tag.1, &labels),
        ),
        (
            by_current_tag.0,
            normalize_note_overviews(by_current_tag.1, &labels),
        ),
        (
            pending_by_current_tag.0,
            normalize_note_overviews(pending_by_current_tag.1, &labels),
        ),
        (
            pending_by_latest_tag.0,
            normalize_note_overviews(pending_by_latest_tag.1, &labels),
        ),
        invalid_view,
    ];

    let current_only = expected_overview(
        "current-only",
        core_kind,
        (1, "approved", true, "current"),
        Some((1, "approved", true, "current")),
        None,
        0,
    );
    let current_plus_unread = expected_overview(
        "current-plus-unread",
        core_kind,
        (1, "approved", true, "current"),
        Some((1, "approved", true, "current")),
        Some((2, "unread", false, "pending")),
        1,
    );
    let current_unread = expected_overview(
        "current-unread",
        core_kind,
        (1, "unread", true, "current-unread"),
        Some((1, "unread", true, "current-unread")),
        Some((1, "unread", true, "current-unread")),
        1,
    );
    let multiple_unread = expected_overview(
        "multiple-unread",
        core_kind,
        (1, "approved", true, "current"),
        Some((1, "approved", true, "current")),
        Some((3, "unread", false, "pending-latest")),
        2,
    );
    let new_unread = expected_overview(
        "new-unread",
        core_kind,
        (1, "unread", false, "new"),
        None,
        Some((1, "unread", false, "new")),
        1,
    );
    let rejected = expected_overview(
        "no-current-rejected",
        core_kind,
        (2, "rejected", false, "rejected"),
        None,
        None,
        0,
    );
    let superseded = expected_overview(
        "no-current-superseded",
        core_kind,
        (2, "superseded", false, "superseded"),
        None,
        None,
        0,
    );
    let version_tag_current = expected_overview(
        "version-tag",
        core_kind,
        (2, "approved", true, "current-tag"),
        Some((2, "approved", true, "current-tag")),
        Some((3, "unread", false, "pending-tag")),
        1,
    );
    let other_kind_current = expected_overview(
        "other-kind-current",
        other_kind,
        (1, "approved", true, "current"),
        Some((1, "approved", true, "current")),
        None,
        0,
    );
    let pending_current_plus_unread = expected_overview(
        "current-plus-unread",
        core_kind,
        (2, "unread", false, "pending"),
        Some((1, "approved", true, "current")),
        Some((2, "unread", false, "pending")),
        1,
    );
    let pending_multiple_unread = expected_overview(
        "multiple-unread",
        core_kind,
        (3, "unread", false, "pending-latest"),
        Some((1, "approved", true, "current")),
        Some((3, "unread", false, "pending-latest")),
        2,
    );
    let pending_new_unread = new_unread.clone();
    let pending_version_tag = expected_overview(
        "version-tag",
        core_kind,
        (3, "unread", false, "pending-tag"),
        Some((2, "approved", true, "current-tag")),
        Some((3, "unread", false, "pending-tag")),
        1,
    );

    let expected_all = json!([
        current_only.clone(),
        current_plus_unread.clone(),
        current_unread.clone(),
        multiple_unread.clone(),
        new_unread.clone(),
        rejected,
        superseded,
        other_kind_current,
        version_tag_current.clone(),
    ]);
    let expected_pending = json!([
        pending_current_plus_unread,
        current_unread.clone(),
        pending_multiple_unread,
        pending_new_unread,
        pending_version_tag,
    ]);
    let expected_by_kind = json!([
        current_only.clone(),
        current_plus_unread.clone(),
        current_unread.clone(),
        multiple_unread.clone(),
        new_unread.clone(),
        expected_overview(
            "no-current-rejected",
            core_kind,
            (2, "rejected", false, "rejected"),
            None,
            None,
            0,
        ),
        expected_overview(
            "no-current-superseded",
            core_kind,
            (2, "superseded", false, "superseded"),
            None,
            None,
            0,
        ),
        version_tag_current.clone(),
    ]);
    let expected = vec![
        (axum::http::StatusCode::OK, expected_all),
        (axum::http::StatusCode::OK, expected_pending),
        (axum::http::StatusCode::OK, expected_by_kind),
        (axum::http::StatusCode::OK, json!([])),
        (axum::http::StatusCode::OK, json!([version_tag_current])),
        (axum::http::StatusCode::OK, json!([])),
        (axum::http::StatusCode::OK, json!([pending_version_tag])),
        (
            axum::http::StatusCode::BAD_REQUEST,
            json!({ "error": "view must be all or pending" }),
        ),
    ];
    assert_eq!(actual, expected);
}
