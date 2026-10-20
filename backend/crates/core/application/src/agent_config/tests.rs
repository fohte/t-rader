use std::collections::BTreeMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, Utc};
use rstest::{fixture, rstest};
use serde_json::json;
use uuid::Uuid;

use crate::change_history::{Actor, ChangeHistoryRecord, FakeChangeHistory, Op, TargetKind};
use crate::persistence::PersistenceError;
use crate::subagent_definition::{
    SharedSubagentDefinitionRepository, SubagentDefinition, SubagentDefinitionRepository,
    SubagentDefinitionRepositoryError,
};
use crate::unit_of_work::{FakeUnitOfWork, UnitOfWorkTransaction};

use super::super::error::{AgentConfigRepositoryError, AgentConfigUseCaseError};
use super::super::repository::AgentConfigRepository;
use super::super::types::{AgentConfig, NewAgentConfig};
use super::AgentConfigUseCases;

#[derive(Default)]
struct FakeAgentConfigRepository {
    configs: Mutex<BTreeMap<String, AgentConfig>>,
}

#[derive(Default)]
struct EmptySubagentDefinitionRepository {
    reference_lock_acquisitions: AtomicUsize,
}

#[async_trait]
impl SubagentDefinitionRepository for EmptySubagentDefinitionRepository {
    async fn list(&self) -> Result<Vec<SubagentDefinition>, SubagentDefinitionRepositoryError> {
        Ok(Vec::new())
    }

    async fn list_in_transaction(
        &self,
        _transaction: &UnitOfWorkTransaction,
    ) -> Result<Vec<SubagentDefinition>, SubagentDefinitionRepositoryError> {
        Ok(Vec::new())
    }

    async fn acquire_agent_graph_reference_lock(
        &self,
        _transaction: &UnitOfWorkTransaction,
    ) -> Result<(), SubagentDefinitionRepositoryError> {
        self.reference_lock_acquisitions
            .fetch_add(1, Ordering::Relaxed);
        Ok(())
    }

    async fn find_by_key(
        &self,
        _key: &str,
    ) -> Result<Option<SubagentDefinition>, SubagentDefinitionRepositoryError> {
        Ok(None)
    }

    async fn find_by_key_in_transaction(
        &self,
        _transaction: &UnitOfWorkTransaction,
        _key: &str,
    ) -> Result<Option<SubagentDefinition>, SubagentDefinitionRepositoryError> {
        Ok(None)
    }

    async fn insert(
        &self,
        _transaction: &UnitOfWorkTransaction,
        _definition: SubagentDefinition,
    ) -> Result<SubagentDefinition, SubagentDefinitionRepositoryError> {
        Err(SubagentDefinitionRepositoryError::InvalidTransaction)
    }

    async fn update(
        &self,
        _transaction: &UnitOfWorkTransaction,
        _definition: SubagentDefinition,
    ) -> Result<SubagentDefinition, SubagentDefinitionRepositoryError> {
        Err(SubagentDefinitionRepositoryError::InvalidTransaction)
    }

    async fn delete(
        &self,
        _transaction: &UnitOfWorkTransaction,
        _key: &str,
    ) -> Result<bool, SubagentDefinitionRepositoryError> {
        Err(SubagentDefinitionRepositoryError::InvalidTransaction)
    }
}

impl FakeAgentConfigRepository {
    fn configs(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, AgentConfig>> {
        self.configs
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[async_trait]
impl AgentConfigRepository for FakeAgentConfigRepository {
    async fn list(&self) -> Result<Vec<AgentConfig>, AgentConfigRepositoryError> {
        Ok(self.configs().values().cloned().collect())
    }

    async fn list_in_transaction(
        &self,
        _transaction: &UnitOfWorkTransaction,
    ) -> Result<Vec<AgentConfig>, AgentConfigRepositoryError> {
        self.list().await
    }

    async fn find_by_purpose(
        &self,
        purpose: &str,
    ) -> Result<Option<AgentConfig>, AgentConfigRepositoryError> {
        Ok(self.configs().get(purpose).cloned())
    }

    async fn find_by_purpose_for_update_in_transaction(
        &self,
        _transaction: &UnitOfWorkTransaction,
        purpose: &str,
    ) -> Result<Option<AgentConfig>, AgentConfigRepositoryError> {
        self.find_by_purpose(purpose).await
    }

    async fn insert(
        &self,
        _transaction: &UnitOfWorkTransaction,
        agent_config: NewAgentConfig,
    ) -> Result<AgentConfig, AgentConfigRepositoryError> {
        let mut configs = self.configs();
        if configs.contains_key(&agent_config.purpose) {
            return Err(PersistenceError::Conflict("duplicate purpose".into()).into());
        }
        let created = AgentConfig {
            id: agent_config.id,
            purpose: agent_config.purpose.clone(),
            agents_md: String::new(),
            skills: json!({}),
            agent_graph: String::new(),
            created_at: fixed_timestamp(),
            updated_at: fixed_timestamp(),
        };
        configs.insert(agent_config.purpose, created.clone());
        Ok(created)
    }

    async fn update_agents_md(
        &self,
        _transaction: &UnitOfWorkTransaction,
        agent_config_id: Uuid,
        agents_md: String,
        updated_at: DateTime<FixedOffset>,
    ) -> Result<AgentConfig, AgentConfigRepositoryError> {
        let mut configs = self.configs();
        let Some(current) = configs
            .values_mut()
            .find(|config| config.id == agent_config_id)
        else {
            return Err(
                PersistenceError::RecordNotUpdated("agent_config disappeared".into()).into(),
            );
        };
        current.agents_md = agents_md;
        current.updated_at = updated_at;
        Ok(current.clone())
    }

    async fn update_skills(
        &self,
        _transaction: &UnitOfWorkTransaction,
        agent_config_id: Uuid,
        skills: serde_json::Value,
        updated_at: DateTime<FixedOffset>,
    ) -> Result<AgentConfig, AgentConfigRepositoryError> {
        let mut configs = self.configs();
        let Some(current) = configs
            .values_mut()
            .find(|config| config.id == agent_config_id)
        else {
            return Err(
                PersistenceError::RecordNotUpdated("agent_config disappeared".into()).into(),
            );
        };
        current.skills = skills;
        current.updated_at = updated_at;
        Ok(current.clone())
    }

    async fn update_agent_graph(
        &self,
        _transaction: &UnitOfWorkTransaction,
        agent_config_id: Uuid,
        agent_graph: String,
        updated_at: DateTime<FixedOffset>,
    ) -> Result<AgentConfig, AgentConfigRepositoryError> {
        let mut configs = self.configs();
        let Some(current) = configs
            .values_mut()
            .find(|config| config.id == agent_config_id)
        else {
            return Err(
                PersistenceError::RecordNotUpdated("agent_config disappeared".into()).into(),
            );
        };
        current.agent_graph = agent_graph;
        current.updated_at = updated_at;
        Ok(current.clone())
    }

    async fn delete(
        &self,
        _transaction: &UnitOfWorkTransaction,
        purpose: &str,
    ) -> Result<bool, AgentConfigRepositoryError> {
        Ok(self.configs().remove(purpose).is_some())
    }
}

struct Fixture {
    use_cases: AgentConfigUseCases,
    unit_of_work: Arc<FakeUnitOfWork>,
    subagent_definition_repository: Arc<EmptySubagentDefinitionRepository>,
    change_history: Arc<FakeChangeHistory>,
}

#[fixture]
fn fixture() -> Fixture {
    let unit_of_work = Arc::new(FakeUnitOfWork::new());
    let repository = Arc::new(FakeAgentConfigRepository::default());
    let subagent_definition_repository = Arc::new(EmptySubagentDefinitionRepository::default());
    let change_history = Arc::new(FakeChangeHistory::new());
    let shared_subagent_definition_repository: SharedSubagentDefinitionRepository =
        subagent_definition_repository.clone();
    Fixture {
        use_cases: AgentConfigUseCases::new(
            unit_of_work.clone(),
            repository,
            shared_subagent_definition_repository,
            change_history.clone(),
        ),
        unit_of_work,
        subagent_definition_repository,
        change_history,
    }
}

fn fixed_timestamp() -> DateTime<FixedOffset> {
    DateTime::<Utc>::UNIX_EPOCH.fixed_offset()
}

fn normalize(mut agent_config: AgentConfig) -> AgentConfig {
    agent_config.id = Uuid::nil();
    agent_config.created_at = fixed_timestamp();
    agent_config.updated_at = fixed_timestamp();
    agent_config
}

async fn get_normalized(use_cases: &AgentConfigUseCases, purpose: &str) -> Option<AgentConfig> {
    use_cases.get(purpose).await.ok().map(normalize)
}

async fn transaction_counts(unit_of_work: &FakeUnitOfWork) -> (usize, usize) {
    (
        unit_of_work.begun.lock().await.len(),
        unit_of_work.committed.lock().await.len(),
    )
}

fn snapshot(
    id: Uuid,
    purpose: &str,
    agents_md: &str,
    skills: serde_json::Value,
    agent_graph: &str,
) -> serde_json::Value {
    json!({
        "id": id,
        "purpose": purpose,
        "agents_md": agents_md,
        "skills": skills,
        "agent_graph": agent_graph,
    })
}

fn expected_snapshot_record(
    op: Op,
    target_id: Uuid,
    before: Option<serde_json::Value>,
    after: Option<serde_json::Value>,
) -> ChangeHistoryRecord {
    ChangeHistoryRecord {
        actor: Actor::Human,
        target_kind: TargetKind::AgentConfig,
        target_id,
        op,
        diff: json!({
            "format": "definition_snapshot",
            "version": 1,
            "before": before,
            "after": after,
        }),
        summary: None,
    }
}

#[rstest]
#[tokio::test]
async fn all_changes_record_full_definition_snapshots(fixture: Fixture) {
    let created = fixture
        .use_cases
        .create("example-purpose".to_string())
        .await
        .expect("create config");
    let id = created.id;
    let initial = snapshot(id, "example-purpose", "", json!({}), "");

    let agents_md = indoc::indoc! {"
        First fabricated line.

        Final fabricated line."};
    fixture
        .use_cases
        .save_agents_md("example-purpose", agents_md.to_string())
        .await
        .expect("save agents_md");
    let with_agents_md = snapshot(id, "example-purpose", agents_md, json!({}), "");

    let first_skill_content = indoc::indoc! {"
        First skill line.

        Final skill line."};
    fixture
        .use_cases
        .put_skill(
            "example-purpose",
            "first_skill",
            first_skill_content.to_string(),
        )
        .await
        .expect("add first skill");
    let with_first_skill = snapshot(
        id,
        "example-purpose",
        agents_md,
        json!({ "first_skill": first_skill_content }),
        "",
    );

    fixture
        .use_cases
        .put_skill(
            "example-purpose",
            "second_skill",
            "second fabricated content".to_string(),
        )
        .await
        .expect("add second skill");
    let with_two_skills = snapshot(
        id,
        "example-purpose",
        agents_md,
        json!({
            "first_skill": first_skill_content,
            "second_skill": "second fabricated content",
        }),
        "",
    );

    fixture
        .use_cases
        .delete_skill("example-purpose", "first_skill")
        .await
        .expect("delete first skill");
    let with_one_skill = snapshot(
        id,
        "example-purpose",
        agents_md,
        json!({ "second_skill": "second fabricated content" }),
        "",
    );

    fixture
        .use_cases
        .put_skills(
            "example-purpose",
            BTreeMap::from([(
                "replacement_skill".to_string(),
                "replacement fabricated content".to_string(),
            )]),
        )
        .await
        .expect("replace skills");
    let replaced_skills = snapshot(
        id,
        "example-purpose",
        agents_md,
        json!({ "replacement_skill": "replacement fabricated content" }),
        "",
    );

    fixture
        .use_cases
        .put_skills("example-purpose", BTreeMap::new())
        .await
        .expect("replace skills with an empty map");
    let empty_skills = snapshot(id, "example-purpose", agents_md, json!({}), "");

    let agent_graph = indoc::indoc! {"
        timeout_minutes: 21
        phases:
          - key: sample-phase
            label: Sample phase
            model: fictional-model-phase
            prompt: fabricated example text"};
    fixture
        .use_cases
        .save_agent_graph("example-purpose", agent_graph)
        .await
        .expect("save agent graph");
    let with_graph = snapshot(id, "example-purpose", agents_md, json!({}), agent_graph);

    fixture
        .use_cases
        .delete("example-purpose")
        .await
        .expect("delete config");

    let records = fixture.change_history.entries.lock().await.clone();
    let committed_transactions = fixture.unit_of_work.committed.lock().await.clone();
    assert_eq!(
        (
            records
                .iter()
                .map(|entry| entry.record.clone())
                .collect::<Vec<_>>(),
            records
                .iter()
                .map(|entry| entry.transaction_id)
                .collect::<Vec<_>>(),
        ),
        (
            vec![
                expected_snapshot_record(Op::Create, id, None, Some(initial.clone())),
                expected_snapshot_record(
                    Op::Update,
                    id,
                    Some(initial),
                    Some(with_agents_md.clone()),
                ),
                expected_snapshot_record(
                    Op::Update,
                    id,
                    Some(with_agents_md),
                    Some(with_first_skill.clone()),
                ),
                expected_snapshot_record(
                    Op::Update,
                    id,
                    Some(with_first_skill),
                    Some(with_two_skills.clone()),
                ),
                expected_snapshot_record(
                    Op::Update,
                    id,
                    Some(with_two_skills),
                    Some(with_one_skill.clone()),
                ),
                expected_snapshot_record(
                    Op::Update,
                    id,
                    Some(with_one_skill),
                    Some(replaced_skills.clone()),
                ),
                expected_snapshot_record(
                    Op::Update,
                    id,
                    Some(replaced_skills),
                    Some(empty_skills.clone()),
                ),
                expected_snapshot_record(
                    Op::Update,
                    id,
                    Some(empty_skills),
                    Some(with_graph.clone()),
                ),
                expected_snapshot_record(Op::Delete, id, Some(with_graph), None),
            ],
            committed_transactions,
        ),
    );
}

#[rstest]
#[case::uppercase("Invalid")]
#[case::space("has space")]
#[case::empty("")]
#[tokio::test]
async fn create_rejects_invalid_purpose(fixture: Fixture, #[case] purpose: &str) {
    let error = fixture
        .use_cases
        .create(purpose.to_string())
        .await
        .expect_err("invalid purpose should be rejected");

    let output = (
        error.to_string(),
        transaction_counts(&fixture.unit_of_work).await,
    );

    assert_eq!(
        output,
        (
            format!("purpose must match ^[a-z0-9][a-z0-9_-]*$ (got '{purpose}')"),
            (0, 0),
        ),
    );
}

#[rstest]
#[tokio::test]
async fn create_and_update_skills_round_trip(fixture: Fixture) {
    fixture
        .use_cases
        .create("example-purpose".to_string())
        .await
        .expect("create agent config");
    fixture
        .use_cases
        .put_skill(
            "example-purpose",
            "sample_skill",
            "initial content".to_string(),
        )
        .await
        .expect("create skill");
    fixture
        .use_cases
        .put_skill(
            "example-purpose",
            "sample_skill",
            "updated content".to_string(),
        )
        .await
        .expect("update skill");

    assert_eq!(
        get_normalized(&fixture.use_cases, "example-purpose").await,
        Some(expected_config(
            "example-purpose",
            "",
            json!({ "sample_skill": "updated content" }),
            "",
        )),
    );
}

#[rstest]
#[tokio::test]
async fn create_maps_repository_conflict_to_duplicate_purpose(fixture: Fixture) {
    fixture
        .use_cases
        .create("example-purpose".to_string())
        .await
        .expect("create agent config");
    let error = fixture
        .use_cases
        .create("example-purpose".to_string())
        .await
        .expect_err("duplicate purpose should be rejected");

    assert_eq!(
        error.to_string(),
        AgentConfigUseCaseError::DuplicatePurpose("example-purpose".to_string()).to_string(),
    );
}

#[rstest]
#[tokio::test]
async fn list_returns_configs_in_purpose_order(fixture: Fixture) {
    fixture
        .use_cases
        .create("zeta-purpose".to_string())
        .await
        .expect("create last config");
    fixture
        .use_cases
        .create("alpha-purpose".to_string())
        .await
        .expect("create first config");

    assert_eq!(
        fixture
            .use_cases
            .list()
            .await
            .expect("list configs")
            .into_iter()
            .map(normalize)
            .collect::<Vec<_>>(),
        vec![
            expected_config("alpha-purpose", "", serde_json::json!({}), ""),
            expected_config("zeta-purpose", "", serde_json::json!({}), ""),
        ],
    );
}

#[rstest]
#[tokio::test]
async fn get_returns_not_found_for_unknown_purpose(fixture: Fixture) {
    let error = fixture
        .use_cases
        .get("missing-purpose")
        .await
        .expect_err("unknown purpose should be rejected");

    assert_eq!(
        error.to_string(),
        "agent_config 'missing-purpose' not found"
    );
}

#[rstest]
#[tokio::test]
async fn delete_removes_config(fixture: Fixture) {
    fixture
        .use_cases
        .create("example-purpose".to_string())
        .await
        .expect("create config");

    fixture
        .use_cases
        .delete("example-purpose")
        .await
        .expect("delete config");
    assert_eq!(
        fixture.use_cases.list().await.expect("list configs"),
        Vec::new(),
    );
}

#[rstest]
#[tokio::test]
async fn delete_returns_not_found_for_unknown_purpose(fixture: Fixture) {
    let error = fixture
        .use_cases
        .delete("missing-purpose")
        .await
        .expect_err("unknown purpose should be rejected");

    assert_eq!(
        error.to_string(),
        "agent_config 'missing-purpose' not found"
    );
}

#[rstest]
#[tokio::test]
async fn save_agents_md_persists_content(fixture: Fixture) {
    fixture
        .use_cases
        .create("example-purpose".to_string())
        .await
        .expect("create config");

    fixture
        .use_cases
        .save_agents_md("example-purpose", "# Guidance".to_string())
        .await
        .expect("save markdown");

    assert_eq!(
        get_normalized(&fixture.use_cases, "example-purpose").await,
        Some(expected_config(
            "example-purpose",
            "# Guidance",
            serde_json::json!({}),
            "",
        )),
    );
}

#[rstest]
#[tokio::test]
async fn put_skills_replaces_the_existing_map(fixture: Fixture) {
    fixture
        .use_cases
        .create("example-purpose".to_string())
        .await
        .expect("create config");
    fixture
        .use_cases
        .put_skill("example-purpose", "old_skill", "old content".to_string())
        .await
        .expect("create initial skill");

    let skills = BTreeMap::from([("new_skill".to_string(), "new content".to_string())]);
    let updated = fixture
        .use_cases
        .put_skills("example-purpose", skills)
        .await
        .expect("replace skills");

    assert_eq!(
        normalize(updated),
        expected_config(
            "example-purpose",
            "",
            serde_json::json!({ "new_skill": "new content" }),
            "",
        ),
    );
}

#[rstest]
#[tokio::test]
async fn put_skills_rejects_invalid_skill_name(fixture: Fixture) {
    let skills = BTreeMap::from([("Invalid Skill".to_string(), "content".to_string())]);
    let error = fixture
        .use_cases
        .put_skills("example-purpose", skills)
        .await
        .expect_err("invalid skill name should be rejected");

    assert_eq!(
        error.to_string(),
        "skill name must match ^[a-z0-9][a-z0-9_-]*$ (got 'Invalid Skill')",
    );
}

#[rstest]
#[tokio::test]
async fn put_skill_upserts_without_removing_other_skills(fixture: Fixture) {
    fixture
        .use_cases
        .create("example-purpose".to_string())
        .await
        .expect("create config");
    fixture
        .use_cases
        .put_skill("example-purpose", "first_skill", "old content".to_string())
        .await
        .expect("create first skill");
    fixture
        .use_cases
        .put_skill(
            "example-purpose",
            "second_skill",
            "keep content".to_string(),
        )
        .await
        .expect("create second skill");

    let updated = fixture
        .use_cases
        .put_skill("example-purpose", "first_skill", "new content".to_string())
        .await
        .expect("update first skill");

    assert_eq!(
        normalize(updated),
        expected_config(
            "example-purpose",
            "",
            serde_json::json!({
                "first_skill": "new content",
                "second_skill": "keep content",
            }),
            "",
        ),
    );
}

#[rstest]
#[tokio::test]
async fn delete_skill_removes_only_the_requested_skill(fixture: Fixture) {
    fixture
        .use_cases
        .create("example-purpose".to_string())
        .await
        .expect("create config");
    fixture
        .use_cases
        .put_skill(
            "example-purpose",
            "removed_skill",
            "remove content".to_string(),
        )
        .await
        .expect("create skill to remove");
    fixture
        .use_cases
        .put_skill("example-purpose", "kept_skill", "keep content".to_string())
        .await
        .expect("create skill to keep");

    let updated = fixture
        .use_cases
        .delete_skill("example-purpose", "removed_skill")
        .await
        .expect("delete skill");

    assert_eq!(
        normalize(updated),
        expected_config(
            "example-purpose",
            "",
            serde_json::json!({ "kept_skill": "keep content" }),
            "",
        ),
    );
}

#[rstest]
#[tokio::test]
async fn delete_skill_returns_not_found_for_unknown_skill(fixture: Fixture) {
    fixture
        .use_cases
        .create("example-purpose".to_string())
        .await
        .expect("create config");

    let error = fixture
        .use_cases
        .delete_skill("example-purpose", "missing_skill")
        .await
        .expect_err("unknown skill should be rejected");

    assert_eq!(error.to_string(), "skill 'missing_skill' not found");
}

#[rstest]
#[tokio::test]
async fn only_agent_graph_updates_acquire_the_reference_lock(fixture: Fixture) {
    fixture
        .use_cases
        .create("example-purpose".to_string())
        .await
        .expect("create config");
    fixture
        .use_cases
        .save_agents_md("example-purpose", "instructions".to_string())
        .await
        .expect("save agents_md");
    fixture
        .use_cases
        .put_skills(
            "example-purpose",
            BTreeMap::from([("sample_skill".to_string(), "skill content".to_string())]),
        )
        .await
        .expect("replace skills");
    fixture
        .use_cases
        .put_skill(
            "example-purpose",
            "sample_skill",
            "updated skill content".to_string(),
        )
        .await
        .expect("upsert skill");
    fixture
        .use_cases
        .delete_skill("example-purpose", "sample_skill")
        .await
        .expect("delete skill");
    let acquisitions_before_graph_update = fixture
        .subagent_definition_repository
        .reference_lock_acquisitions
        .load(Ordering::Relaxed);

    let yaml = indoc::indoc! {"
        phases:
          - key: sample_phase
            label: Sample phase
            model: sample-model-plan
            prompt: Sample prompt
    "};
    fixture
        .use_cases
        .save_agent_graph("example-purpose", yaml)
        .await
        .expect("save graph");
    let acquisitions_after_graph_update = fixture
        .subagent_definition_repository
        .reference_lock_acquisitions
        .load(Ordering::Relaxed);

    assert_eq!(
        (
            acquisitions_before_graph_update,
            acquisitions_after_graph_update,
        ),
        (0, 1),
    );
}

#[rstest]
#[tokio::test]
async fn save_agent_graph_validates_then_persists_yaml(fixture: Fixture) {
    fixture
        .use_cases
        .create("example-purpose".to_string())
        .await
        .expect("create config");
    let yaml = indoc::indoc! {"
        phases:
          - key: sample_phase
            label: Sample phase
            model: sample-model-plan
            prompt: Sample prompt
    "};

    let result = fixture
        .use_cases
        .save_agent_graph("example-purpose", yaml)
        .await
        .expect("save graph");
    let config = get_normalized(&fixture.use_cases, "example-purpose").await;

    assert_eq!(
        (result, config),
        (
            yaml.to_string(),
            Some(expected_config(
                "example-purpose",
                "",
                serde_json::json!({}),
                yaml,
            )),
        ),
    );
}

#[rstest]
#[tokio::test]
async fn save_agent_graph_rejects_invalid_yaml_without_saving(fixture: Fixture) {
    fixture
        .use_cases
        .create("example-purpose".to_string())
        .await
        .expect("create config");
    let error = fixture
        .use_cases
        .save_agent_graph("example-purpose", "phases: [")
        .await
        .expect_err("invalid graph should be rejected");

    let output = (
        error.to_string(),
        get_normalized(&fixture.use_cases, "example-purpose").await,
        transaction_counts(&fixture.unit_of_work).await,
    );

    assert_eq!(
        output,
        (
            "agent_graph is not valid YAML: did not find expected node content at line 2 column 1, while parsing a flow node".to_string(),
            Some(expected_config(
                "example-purpose",
                "",
                serde_json::json!({}),
                "",
            )),
            (1, 1),
        ),
    );
}

fn expected_config(
    purpose: &str,
    agents_md: &str,
    skills: serde_json::Value,
    agent_graph: &str,
) -> AgentConfig {
    AgentConfig {
        id: Uuid::nil(),
        purpose: purpose.to_string(),
        agents_md: agents_md.to_string(),
        skills,
        agent_graph: agent_graph.to_string(),
        created_at: fixed_timestamp(),
        updated_at: fixed_timestamp(),
    }
}
