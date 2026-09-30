use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset, Utc};
use rstest::{fixture, rstest};
use serde_json::json;
use uuid::Uuid;

use crate::persistence::PersistenceError;
use crate::unit_of_work::{FakeUnitOfWork, UnitOfWorkTransaction};

use super::super::error::{AgentConfigRepositoryError, AgentConfigUseCaseError};
use super::super::repository::AgentConfigRepository;
use super::super::types::{AgentConfig, NewAgentConfig};
use super::AgentConfigUseCases;

#[derive(Default)]
struct FakeAgentConfigRepository {
    configs: Mutex<BTreeMap<String, AgentConfig>>,
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

    async fn find_by_purpose(
        &self,
        purpose: &str,
    ) -> Result<Option<AgentConfig>, AgentConfigRepositoryError> {
        Ok(self.configs().get(purpose).cloned())
    }

    async fn find_by_purpose_in_transaction(
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

    async fn update(
        &self,
        _transaction: &UnitOfWorkTransaction,
        agent_config: AgentConfig,
    ) -> Result<AgentConfig, AgentConfigRepositoryError> {
        let mut configs = self.configs();
        if !configs.contains_key(&agent_config.purpose) {
            return Err(
                PersistenceError::RecordNotUpdated("agent_config disappeared".into()).into(),
            );
        }
        configs.insert(agent_config.purpose.clone(), agent_config.clone());
        Ok(agent_config)
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
}

#[fixture]
fn fixture() -> Fixture {
    let unit_of_work = Arc::new(FakeUnitOfWork::new());
    let repository = Arc::new(FakeAgentConfigRepository::default());
    Fixture {
        use_cases: AgentConfigUseCases::new(unit_of_work.clone(), repository),
        unit_of_work,
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
