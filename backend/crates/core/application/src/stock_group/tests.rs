use std::sync::Arc;

use crate::change_history::{Actor, FakeChangeHistory, Op, TargetKind};
use crate::unit_of_work::{FakeUnitOfWork, SharedUnitOfWork};
use rstest::{fixture, rstest};
use serde_json::json;

use super::{
    CreateStockGroupCommand, FakeStockGroupRepository, StockGroup, StockGroupUseCaseError,
    StockGroupUseCases, UpdateStockGroupCommand,
};
use crate::stock_group::SharedStockGroupRepository;

struct Harness {
    use_cases: StockGroupUseCases,
    repository: Arc<FakeStockGroupRepository>,
    unit_of_work: Arc<FakeUnitOfWork>,
    change_history: Arc<FakeChangeHistory>,
}

#[fixture]
fn harness() -> Harness {
    let unit_of_work = Arc::new(FakeUnitOfWork::new());
    let repository = Arc::new(FakeStockGroupRepository::new());
    let change_history = Arc::new(FakeChangeHistory::new());
    let shared_unit_of_work: SharedUnitOfWork = unit_of_work.clone();
    let shared_repository: SharedStockGroupRepository = repository.clone();
    let shared_change_history = change_history.clone();

    Harness {
        use_cases: StockGroupUseCases::new(
            shared_unit_of_work,
            shared_repository,
            shared_change_history,
        ),
        repository,
        unit_of_work,
        change_history,
    }
}

async fn create_group(harness: &Harness) -> Result<StockGroup, StockGroupUseCaseError> {
    harness.repository.insert_axis("sample-axis", None).await;
    harness
        .use_cases
        .create(CreateStockGroupCommand {
            axis_key: "sample-axis".into(),
            key: "sample-group".into(),
            name: "Sample group".into(),
            description: Some("Sample description".into()),
        })
        .await
}

#[rstest]
#[tokio::test]
async fn create_records_history_in_the_same_transaction(harness: Harness) {
    harness.repository.insert_axis("sample-axis", None).await;
    let group = harness
        .use_cases
        .create(CreateStockGroupCommand {
            axis_key: "sample-axis".into(),
            key: "sample-group".into(),
            name: "Sample group".into(),
            description: Some("Sample description".into()),
        })
        .await
        .expect("group creation succeeds");
    let transaction_id = harness
        .unit_of_work
        .committed
        .lock()
        .await
        .first()
        .copied()
        .unwrap_or_default();
    let committed = harness.unit_of_work.committed.lock().await.clone();
    let repository_transactions = harness.repository.transaction_ids.lock().await.clone();
    let history = harness.change_history.entries.lock().await.clone();

    assert_eq!(
        (
            group.clone(),
            committed,
            repository_transactions
                .iter()
                .map(|id| *id == transaction_id)
                .collect::<Vec<_>>(),
            history
                .into_iter()
                .map(|entry| (
                    entry.transaction_id == transaction_id,
                    entry.record.actor,
                    entry.record.target_kind,
                    entry.record.target_id,
                    entry.record.op,
                    entry.record.diff,
                    entry.record.summary,
                ))
                .collect::<Vec<_>>(),
        ),
        (
            StockGroup {
                id: group.id,
                axis_id: group.axis_id,
                axis_key: "sample-axis".into(),
                key: "sample-group".into(),
                name: "Sample group".into(),
                description: Some("Sample description".into()),
            },
            vec![transaction_id],
            vec![true; 3],
            vec![(
                true,
                Actor::Llm { label: "analyst" },
                TargetKind::StockGroup,
                group.id,
                Op::Create,
                json!({
                    "axis_key": "sample-axis",
                    "group_key": "sample-group",
                    "name": "Sample group",
                    "description": "Sample description",
                }),
                None,
            )],
        ),
    );
}

#[rstest]
#[tokio::test]
async fn update_records_only_changed_fields_and_keeps_keys_immutable(harness: Harness) {
    let group = create_group(&harness)
        .await
        .expect("group creation succeeds");
    harness.change_history.entries.lock().await.clear();
    let updated = harness
        .use_cases
        .update(UpdateStockGroupCommand {
            axis_key: "sample-axis".into(),
            key: "sample-group".into(),
            name: Some("Renamed group".into()),
            description: Some(None),
        })
        .await
        .expect("group update succeeds");
    let history = harness.change_history.entries.lock().await.clone();

    assert_eq!(
        (
            updated,
            history
                .into_iter()
                .map(|entry| (
                    entry.record.actor,
                    entry.record.target_kind,
                    entry.record.target_id,
                    entry.record.op,
                    entry.record.diff,
                ))
                .collect::<Vec<_>>(),
        ),
        (
            StockGroup {
                id: group.id,
                axis_id: group.axis_id,
                axis_key: "sample-axis".into(),
                key: "sample-group".into(),
                name: "Renamed group".into(),
                description: None,
            },
            vec![(
                Actor::Llm { label: "analyst" },
                TargetKind::StockGroup,
                group.id,
                Op::Update,
                json!({
                    "description": { "from": "Sample description", "to": null },
                    "name": { "from": "Sample group", "to": "Renamed group" },
                }),
            )],
        ),
    );
}

#[rstest]
#[tokio::test]
async fn membership_changes_are_idempotent_audited_and_listed(harness: Harness) {
    let group = create_group(&harness)
        .await
        .expect("group creation succeeds");
    harness.change_history.entries.lock().await.clear();
    harness.repository.insert_stock("0001").await;
    harness.repository.insert_stock("0002").await;

    let added_first = harness
        .use_cases
        .add_stock("sample-axis", "sample-group", "0002")
        .await
        .expect("first add succeeds");
    let added_second = harness
        .use_cases
        .add_stock("sample-axis", "sample-group", "0002")
        .await
        .expect("repeated add succeeds");
    harness
        .use_cases
        .add_stock("sample-axis", "sample-group", "0001")
        .await
        .expect("second stock add succeeds");
    let stocks = harness
        .use_cases
        .list_stock_ids("sample-axis", "sample-group")
        .await
        .expect("stock listing succeeds");
    let removed_first = harness
        .use_cases
        .remove_stock("sample-axis", "sample-group", "0002")
        .await
        .expect("first remove succeeds");
    let removed_second = harness
        .use_cases
        .remove_stock("sample-axis", "sample-group", "0002")
        .await
        .expect("repeated remove succeeds");
    let remaining = harness
        .use_cases
        .list_stock_ids("sample-axis", "sample-group")
        .await
        .expect("stock listing succeeds");
    let history = harness.change_history.entries.lock().await.clone();

    assert_eq!(
        (
            added_first,
            added_second,
            stocks,
            removed_first,
            removed_second,
            remaining,
            history
                .into_iter()
                .map(|entry| (
                    entry.record.actor,
                    entry.record.target_kind,
                    entry.record.target_id,
                    entry.record.op,
                    entry.record.diff,
                ))
                .collect::<Vec<_>>(),
        ),
        (
            true,
            false,
            vec!["0001".to_string(), "0002".to_string()],
            true,
            false,
            vec!["0001".to_string()],
            vec![
                (
                    Actor::Llm { label: "analyst" },
                    TargetKind::StockGroup,
                    group.id,
                    Op::Update,
                    json!({
                        "stock_id": "0002",
                        "membership": { "from": false, "to": true },
                    }),
                ),
                (
                    Actor::Llm { label: "analyst" },
                    TargetKind::StockGroup,
                    group.id,
                    Op::Update,
                    json!({
                        "stock_id": "0001",
                        "membership": { "from": false, "to": true },
                    }),
                ),
                (
                    Actor::Llm { label: "analyst" },
                    TargetKind::StockGroup,
                    group.id,
                    Op::Update,
                    json!({
                        "stock_id": "0002",
                        "membership": { "from": true, "to": false },
                    }),
                ),
            ],
        ),
    );
}

#[rstest]
#[tokio::test]
async fn synchronized_axes_reject_group_and_membership_mutations(harness: Harness) {
    harness
        .repository
        .insert_axis("sample-axis", Some("sample-sync"))
        .await;

    let errors = vec![
        harness
            .use_cases
            .create(CreateStockGroupCommand {
                axis_key: "sample-axis".into(),
                key: "sample-group".into(),
                name: "Sample group".into(),
                description: None,
            })
            .await
            .expect_err("synchronized axis rejects group creation")
            .to_string(),
        harness
            .use_cases
            .update(UpdateStockGroupCommand {
                axis_key: "sample-axis".into(),
                key: "sample-group".into(),
                name: Some("Renamed group".into()),
                description: None,
            })
            .await
            .expect_err("synchronized axis rejects group update")
            .to_string(),
        harness
            .use_cases
            .add_stock("sample-axis", "sample-group", "0001")
            .await
            .expect_err("synchronized axis rejects member addition")
            .to_string(),
        harness
            .use_cases
            .remove_stock("sample-axis", "sample-group", "0001")
            .await
            .expect_err("synchronized axis rejects member removal")
            .to_string(),
    ];

    assert_eq!(
        (
            errors,
            harness.change_history.entries.lock().await.len(),
            harness.unit_of_work.committed.lock().await.len(),
        ),
        (
            vec!["stock groups on synchronized axes cannot be changed by MCP".to_string(); 4],
            0,
            0,
        ),
    );
}
