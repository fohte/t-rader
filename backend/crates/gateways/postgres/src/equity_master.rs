use std::collections::{HashMap, HashSet};

use async_trait::async_trait;
use chrono::Utc;
use core_application::equity_master::{EquityMasterRepository, EquityMasterRepositoryError};
use core_application::unit_of_work::UnitOfWorkTransaction;
use core_domain::equity_master::EquityMasterEntry;
use sea_orm::ActiveValue::{NotSet, Set};
use sea_orm::sea_query::OnConflict;
use sea_orm::{ColumnTrait, Condition, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::entities::{group_axis, sector, stock, stock_group, stock_group_member};
use crate::persistence::persistence_error;
use crate::transaction::transaction_ref as postgres_transaction_ref;

#[derive(Clone)]
pub struct PostgresEquityMasterRepository;

#[async_trait]
impl EquityMasterRepository for PostgresEquityMasterRepository {
    async fn upsert(
        &self,
        transaction: &UnitOfWorkTransaction,
        entries: &[EquityMasterEntry],
    ) -> Result<usize, EquityMasterRepositoryError> {
        let transaction = postgres_transaction_ref(transaction)
            .ok_or(EquityMasterRepositoryError::InvalidTransaction)?;
        let sectors = entries
            .iter()
            .filter_map(|entry| entry.sector_name.as_deref())
            .collect::<HashSet<_>>();
        if !sectors.is_empty() {
            let models = sectors.into_iter().map(|name| sector::ActiveModel {
                id: Set(name.to_owned()),
                name: Set(name.to_owned()),
            });
            // ON CONFLICT DO NOTHING では RETURNING 行がないため、exec_without_returning を使う。
            sector::Entity::insert_many(models)
                .on_conflict(
                    OnConflict::column(sector::Column::Id)
                        .do_nothing()
                        .to_owned(),
                )
                .exec_without_returning(transaction)
                .await
                .map_err(repository_error)?;
        }

        let now = Utc::now().fixed_offset();
        let models = entries.iter().map(|entry| stock::ActiveModel {
            id: Set(entry.id.clone()),
            name: Set(entry.name.clone()),
            market: Set(entry.market.clone()),
            sector_id: Set(entry.sector_name.clone()),
            product_category: Set(entry.product_category.clone()),
            created_at: Set(now),
            updated_at: Set(now),
        });
        stock::Entity::insert_many(models)
            .on_conflict(
                OnConflict::column(stock::Column::Id)
                    .update_columns([
                        stock::Column::Name,
                        stock::Column::Market,
                        stock::Column::SectorId,
                        stock::Column::ProductCategory,
                        stock::Column::UpdatedAt,
                    ])
                    .to_owned(),
            )
            .exec_without_returning(transaction)
            .await
            .map_err(repository_error)?;

        sync_sector_groups(transaction, entries).await?;

        Ok(entries.len())
    }
}

async fn sync_sector_groups(
    transaction: &sea_orm::DatabaseTransaction,
    entries: &[EquityMasterEntry],
) -> Result<(), EquityMasterRepositoryError> {
    let axes = group_axis::Entity::find()
        .filter(group_axis::Column::SyncSource.eq("jquants"))
        .all(transaction)
        .await
        .map_err(repository_error)?;
    if axes.is_empty() {
        return Ok(());
    }

    let sector_names = entries
        .iter()
        .filter_map(|entry| entry.sector_name.as_deref())
        .collect::<HashSet<_>>();

    for axis in axes {
        sync_axis_sector_groups(transaction, axis.id, entries, &sector_names).await?;
    }

    Ok(())
}

async fn sync_axis_sector_groups(
    transaction: &sea_orm::DatabaseTransaction,
    axis_id: Uuid,
    entries: &[EquityMasterEntry],
    sector_names: &HashSet<&str>,
) -> Result<(), EquityMasterRepositoryError> {
    upsert_sector_groups(transaction, axis_id, sector_names).await?;

    let groups = stock_group::Entity::find()
        .filter(stock_group::Column::AxisId.eq(axis_id))
        .all(transaction)
        .await
        .map_err(repository_error)?;
    if groups.is_empty() {
        return Ok(());
    }

    let groups_by_key = groups
        .iter()
        .map(|group| (group.key.as_str(), group.id))
        .collect::<HashMap<_, _>>();
    let desired_groups_by_stock = entries
        .iter()
        .filter_map(|entry| {
            entry
                .sector_name
                .as_deref()
                .and_then(|key| groups_by_key.get(key))
                .map(|group_id| (entry.id.as_str(), *group_id))
        })
        .collect::<HashMap<_, _>>();

    sync_axis_memberships(transaction, entries, &groups, &desired_groups_by_stock).await
}

async fn upsert_sector_groups(
    transaction: &sea_orm::DatabaseTransaction,
    axis_id: Uuid,
    sector_names: &HashSet<&str>,
) -> Result<(), EquityMasterRepositoryError> {
    if sector_names.is_empty() {
        return Ok(());
    }

    let groups = sector_names.iter().map(|name| stock_group::ActiveModel {
        id: Set(Uuid::new_v4()),
        axis_id: Set(axis_id),
        key: Set((*name).to_owned()),
        name: Set((*name).to_owned()),
        description: Set(None),
    });
    stock_group::Entity::insert_many(groups)
        .on_conflict(
            OnConflict::columns([stock_group::Column::AxisId, stock_group::Column::Key])
                .update_column(stock_group::Column::Name)
                .to_owned(),
        )
        .exec_without_returning(transaction)
        .await
        .map_err(repository_error)?;

    Ok(())
}

async fn sync_axis_memberships(
    transaction: &sea_orm::DatabaseTransaction,
    entries: &[EquityMasterEntry],
    groups: &[stock_group::Model],
    desired_groups_by_stock: &HashMap<&str, Uuid>,
) -> Result<(), EquityMasterRepositoryError> {
    let group_ids = groups.iter().map(|group| group.id).collect::<Vec<_>>();
    let stock_ids = entries
        .iter()
        .map(|entry| entry.id.as_str())
        .collect::<Vec<_>>();
    let existing_members = stock_group_member::Entity::find()
        .filter(stock_group_member::Column::StockId.is_in(stock_ids))
        .filter(stock_group_member::Column::GroupId.is_in(group_ids))
        .all(transaction)
        .await
        .map_err(repository_error)?;
    let stale_members = existing_members
        .iter()
        .filter(|member| {
            desired_groups_by_stock.get(member.stock_id.as_str()) != Some(&member.group_id)
        })
        .collect::<Vec<_>>();
    if !stale_members.is_empty() {
        let stale_condition = stale_members
            .iter()
            .fold(Condition::any(), |condition, member| {
                condition.add(
                    Condition::all()
                        .add(stock_group_member::Column::StockId.eq(member.stock_id.clone()))
                        .add(stock_group_member::Column::GroupId.eq(member.group_id)),
                )
            });
        stock_group_member::Entity::delete_many()
            .filter(stale_condition)
            .exec(transaction)
            .await
            .map_err(repository_error)?;
    }

    if desired_groups_by_stock.is_empty() {
        return Ok(());
    }

    let memberships = desired_groups_by_stock.iter().map(|(stock_id, group_id)| {
        stock_group_member::ActiveModel {
            stock_id: Set((*stock_id).to_owned()),
            group_id: Set(*group_id),
            created_at: NotSet,
        }
    });
    stock_group_member::Entity::insert_many(memberships)
        .on_conflict(
            OnConflict::columns([
                stock_group_member::Column::StockId,
                stock_group_member::Column::GroupId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(transaction)
        .await
        .map_err(repository_error)?;

    Ok(())
}

fn repository_error(error: sea_orm::DbErr) -> EquityMasterRepositoryError {
    EquityMasterRepositoryError::Database(persistence_error(error))
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use core_application::equity_master::EquityMasterRepository;
    use core_application::unit_of_work::UnitOfWork;
    use core_domain::equity_master::EquityMasterEntry;
    use sea_orm::ActiveValue::{NotSet, Set};
    use sea_orm::EntityTrait;

    use super::PostgresEquityMasterRepository;
    use crate::entities::{group_axis, stock, stock_group, stock_group_member};
    use crate::{DatabaseHandle, PostgresUnitOfWork};

    type GroupSnapshot = Vec<(String, String, String)>;
    type MemberSnapshot = Vec<(String, String, String)>;

    fn entry(id: &str, sector_name: Option<&str>) -> EquityMasterEntry {
        EquityMasterEntry {
            id: id.to_owned(),
            name: format!("架空銘柄{id}"),
            market: Some("架空市場".to_owned()),
            sector_name: sector_name.map(str::to_owned),
            product_category: Some("000".to_owned()),
        }
    }

    async fn insert_axis(db: &DatabaseHandle, key: &str, sync_source: Option<&str>) -> uuid::Uuid {
        group_axis::Entity::insert(group_axis::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            key: Set(key.to_owned()),
            name: Set(format!("架空分類軸 {key}")),
            description: Set("架空の分類軸".to_owned()),
            sync_source: Set(sync_source.map(str::to_owned)),
        })
        .exec_with_returning(db)
        .await
        .expect("insert group axis")
        .id
    }

    async fn insert_group(
        db: &DatabaseHandle,
        axis_id: uuid::Uuid,
        key: &str,
        name: &str,
    ) -> uuid::Uuid {
        stock_group::Entity::insert(stock_group::ActiveModel {
            id: Set(uuid::Uuid::new_v4()),
            axis_id: Set(axis_id),
            key: Set(key.to_owned()),
            name: Set(name.to_owned()),
            description: Set(None),
        })
        .exec_with_returning(db)
        .await
        .expect("insert stock group")
        .id
    }

    async fn insert_membership(db: &DatabaseHandle, group_id: uuid::Uuid, stock_id: &str) {
        stock_group_member::Entity::insert(stock_group_member::ActiveModel {
            stock_id: Set(stock_id.to_owned()),
            group_id: Set(group_id),
            created_at: NotSet,
        })
        .exec_without_returning(db)
        .await
        .expect("insert stock group membership");
    }

    async fn upsert(db: &DatabaseHandle, entries: &[EquityMasterEntry]) -> usize {
        let unit_of_work = PostgresUnitOfWork::new(db.clone());
        let transaction = unit_of_work.begin().await.expect("begin transaction");
        let upserted = PostgresEquityMasterRepository
            .upsert(&transaction, entries)
            .await
            .expect("upsert equity master");
        unit_of_work
            .commit(transaction)
            .await
            .expect("commit transaction");
        upserted
    }

    async fn snapshot(db: &DatabaseHandle) -> (GroupSnapshot, MemberSnapshot) {
        let axes = group_axis::Entity::find()
            .all(db)
            .await
            .expect("find group axes");
        let axis_keys = axes
            .into_iter()
            .map(|axis| (axis.id, axis.key))
            .collect::<HashMap<_, _>>();
        let groups = stock_group::Entity::find()
            .all(db)
            .await
            .expect("find stock groups");
        let group_axis_keys = groups
            .iter()
            .map(|group| {
                (
                    group.id,
                    (
                        axis_keys
                            .get(&group.axis_id)
                            .expect("stock group axis exists")
                            .clone(),
                        group.key.clone(),
                    ),
                )
            })
            .collect::<HashMap<_, _>>();
        let mut group_snapshot = groups
            .into_iter()
            .map(|group| {
                (
                    axis_keys
                        .get(&group.axis_id)
                        .expect("stock group axis exists")
                        .clone(),
                    group.key,
                    group.name,
                )
            })
            .collect::<GroupSnapshot>();
        group_snapshot.sort();

        let members = stock_group_member::Entity::find()
            .all(db)
            .await
            .expect("find stock group memberships");
        let mut member_snapshot = members
            .into_iter()
            .map(|member| {
                let (axis_key, group_key) = group_axis_keys
                    .get(&member.group_id)
                    .expect("membership group exists");
                (member.stock_id, axis_key.clone(), group_key.clone())
            })
            .collect::<MemberSnapshot>();
        member_snapshot.sort();

        (group_snapshot, member_snapshot)
    }

    fn expected_group(axis: &str, key: &str) -> (String, String, String) {
        (axis.to_owned(), key.to_owned(), key.to_owned())
    }

    fn expected_member(stock: &str, axis: &str, group: &str) -> (String, String, String) {
        (stock.to_owned(), axis.to_owned(), group.to_owned())
    }

    fn expected_snapshot(
        mut groups: GroupSnapshot,
        mut members: MemberSnapshot,
    ) -> (GroupSnapshot, MemberSnapshot) {
        groups.sort();
        members.sort();
        (groups, members)
    }

    #[backend_test_macros::database_test]
    async fn sync_assigns_stock_to_group_on_each_jquants_axis(db: DatabaseHandle) {
        insert_axis(&db, "synthetic-axis-a", Some("jquants")).await;
        insert_axis(&db, "synthetic-axis-b", Some("jquants")).await;
        let entries = [
            entry("ZZ91", Some("架空業種A")),
            entry("ZZ92", Some("架空業種B")),
            entry("ZZ93", None),
        ];

        let count = upsert(&db, &entries).await;
        let actual = snapshot(&db).await;

        assert_eq!(
            (count, actual),
            (
                3,
                expected_snapshot(
                    vec![
                        expected_group("synthetic-axis-a", "架空業種A"),
                        expected_group("synthetic-axis-a", "架空業種B"),
                        expected_group("synthetic-axis-b", "架空業種A"),
                        expected_group("synthetic-axis-b", "架空業種B"),
                    ],
                    vec![
                        expected_member("ZZ91", "synthetic-axis-a", "架空業種A"),
                        expected_member("ZZ91", "synthetic-axis-b", "架空業種A"),
                        expected_member("ZZ92", "synthetic-axis-a", "架空業種B"),
                        expected_member("ZZ92", "synthetic-axis-b", "架空業種B"),
                    ],
                ),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn repeated_sync_leaves_groups_and_members_unchanged(db: DatabaseHandle) {
        insert_axis(&db, "synthetic-axis-a", Some("jquants")).await;
        let entries = [entry("ZZ91", Some("架空業種A"))];

        let initial_count = upsert(&db, &entries).await;
        let before_repeat = snapshot(&db).await;
        let repeated_count = upsert(&db, &entries).await;
        let after_repeat = snapshot(&db).await;

        let expected = expected_snapshot(
            vec![expected_group("synthetic-axis-a", "架空業種A")],
            vec![expected_member("ZZ91", "synthetic-axis-a", "架空業種A")],
        );
        assert_eq!(
            (initial_count, repeated_count, before_repeat, after_repeat),
            (1, 1, expected.clone(), expected),
        );
    }

    #[backend_test_macros::database_test]
    async fn sync_moves_member_when_sector_changes_and_removes_when_sector_is_none(
        db: DatabaseHandle,
    ) {
        insert_axis(&db, "synthetic-axis-a", Some("jquants")).await;
        upsert(&db, &[entry("ZZ91", Some("架空業種A"))]).await;

        let move_count = upsert(&db, &[entry("ZZ91", Some("架空業種B"))]).await;
        let after_move = snapshot(&db).await;
        let removal_count = upsert(&db, &[entry("ZZ91", None)]).await;
        let after_removal = snapshot(&db).await;

        assert_eq!(
            (move_count, after_move, removal_count, after_removal),
            (
                1,
                expected_snapshot(
                    vec![
                        expected_group("synthetic-axis-a", "架空業種A"),
                        expected_group("synthetic-axis-a", "架空業種B"),
                    ],
                    vec![expected_member("ZZ91", "synthetic-axis-a", "架空業種B",)],
                ),
                1,
                expected_snapshot(
                    vec![
                        expected_group("synthetic-axis-a", "架空業種A"),
                        expected_group("synthetic-axis-a", "架空業種B"),
                    ],
                    vec![],
                ),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn sync_keeps_memberships_on_axes_without_jquants_source(db: DatabaseHandle) {
        insert_axis(&db, "synthetic-axis-a", Some("jquants")).await;
        let manual_axis_id = insert_axis(&db, "synthetic-manual-axis", None).await;
        let other_axis_id =
            insert_axis(&db, "synthetic-other-axis", Some("synthetic-source")).await;
        upsert(&db, &[entry("ZZ92", Some("架空業種A"))]).await;

        let manual_group_id =
            insert_group(&db, manual_axis_id, "架空手動分類", "架空手動分類").await;
        let other_group_id = insert_group(&db, other_axis_id, "架空外部分類", "架空外部分類").await;
        insert_membership(&db, manual_group_id, "ZZ92").await;
        insert_membership(&db, other_group_id, "ZZ92").await;

        let count = upsert(&db, &[entry("ZZ92", Some("架空業種B"))]).await;
        let actual = snapshot(&db).await;

        assert_eq!(
            (count, actual),
            (
                1,
                expected_snapshot(
                    vec![
                        expected_group("synthetic-axis-a", "架空業種A"),
                        expected_group("synthetic-axis-a", "架空業種B"),
                        expected_group("synthetic-manual-axis", "架空手動分類"),
                        expected_group("synthetic-other-axis", "架空外部分類"),
                    ],
                    vec![
                        expected_member("ZZ92", "synthetic-axis-a", "架空業種B"),
                        expected_member("ZZ92", "synthetic-manual-axis", "架空手動分類"),
                        expected_member("ZZ92", "synthetic-other-axis", "架空外部分類"),
                    ],
                ),
            ),
        );
    }

    #[backend_test_macros::database_test]
    async fn sync_without_jquants_axis_only_updates_stock_master(db: DatabaseHandle) {
        let manual_axis_id = insert_axis(&db, "synthetic-manual-axis", None).await;
        let other_axis_id =
            insert_axis(&db, "synthetic-other-axis", Some("synthetic-source")).await;
        let initial_entries = [entry("ZZ94", Some("架空業種A"))];
        let initial_count = upsert(&db, &initial_entries).await;

        let manual_group_id =
            insert_group(&db, manual_axis_id, "架空手動分類", "架空手動分類").await;
        let other_group_id = insert_group(&db, other_axis_id, "架空外部分類", "架空外部分類").await;
        insert_membership(&db, manual_group_id, "ZZ94").await;
        insert_membership(&db, other_group_id, "ZZ94").await;

        let updated_entries = [entry("ZZ94", Some("架空業種B"))];
        let updated_count = upsert(&db, &updated_entries).await;

        let actual = snapshot(&db).await;
        let stock_sector = stock::Entity::find_by_id("ZZ94".to_owned())
            .one(&db)
            .await
            .expect("find updated stock")
            .and_then(|stock| stock.sector_id);
        let expected = (
            vec![
                expected_group("synthetic-manual-axis", "架空手動分類"),
                expected_group("synthetic-other-axis", "架空外部分類"),
            ],
            vec![
                expected_member("ZZ94", "synthetic-manual-axis", "架空手動分類"),
                expected_member("ZZ94", "synthetic-other-axis", "架空外部分類"),
            ],
        );
        assert_eq!(
            (initial_count, updated_count, actual, stock_sector),
            (1, 1, expected, Some("架空業種B".to_owned())),
        );
    }
}
