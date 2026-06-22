//! 戦略 Agent reconcile で apply する Kubernetes リソースのマニフェスト構築。

use std::collections::BTreeMap;

use indoc::formatdoc;
use serde_json::{Value, json};
use uuid::Uuid;

pub const AGENT_API_VERSION: &str = "kubeopencode.io/v1alpha1";
pub const AGENT_KIND: &str = "Agent";
pub const AGENT_GROUP: &str = "kubeopencode.io";
pub const AGENT_VERSION: &str = "v1alpha1";
pub const AGENT_PLURAL: &str = "agents";

pub const SA_API_VERSION: &str = "v1";
pub const SA_KIND: &str = "ServiceAccount";
pub const SA_PLURAL: &str = "serviceaccounts";

pub const CONFIGMAP_API_VERSION: &str = "v1";
pub const CONFIGMAP_KIND: &str = "ConfigMap";
pub const CONFIGMAP_PLURAL: &str = "configmaps";

pub const EXTERNAL_SECRET_API_VERSION: &str = "external-secrets.io/v1";
pub const EXTERNAL_SECRET_KIND: &str = "ExternalSecret";
pub const EXTERNAL_SECRET_GROUP: &str = "external-secrets.io";
pub const EXTERNAL_SECRET_VERSION: &str = "v1";
pub const EXTERNAL_SECRET_PLURAL: &str = "externalsecrets";

/// SSA fieldManager。`force=true` で同じ manager の field を上書きする。
pub const FIELD_MANAGER: &str = "t-rader-backend";

pub const DEFAULT_AGENT_MODEL: &str = "opencode-go/minimax-m3";
pub const DEFAULT_AGENT_SMALL_MODEL: &str = "opencode-go/deepseek-v4-flash";
/// 1 件の戦略 Agent につき発行する SSM パラメータ key の template。
/// `{name}` が `strategy-{uuid_no_dashes}` で置換される。
pub const DEFAULT_SSM_PARAMETER_TEMPLATE: &str = "/infra/kubeopencode/{name}-opencode-api-key";

/// 戦略 Agent 名を生成する。UUID のハイフンを除いた 32 文字を suffix にする
/// (RFC 1123 label を満たす範囲)。
pub fn agent_name_for(strategy_id: Uuid) -> String {
    format!("strategy-{}", strategy_id.simple())
}

/// DB から取り出した 1 戦略分の reconcile 入力。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrategyAgentSpec {
    pub strategy_id: Uuid,
    pub agent_name: String,
    pub agents_md: String,
    /// skill 名 → markdown 本文。ConfigMap key と Agent の volumeMount path に使う。
    pub skills: BTreeMap<String, String>,
}

impl StrategyAgentSpec {
    pub fn new(
        strategy_id: Uuid,
        agents_md: impl Into<String>,
        skills: BTreeMap<String, String>,
    ) -> Self {
        Self {
            strategy_id,
            agent_name: agent_name_for(strategy_id),
            agents_md: agents_md.into(),
            skills,
        }
    }
}

/// クラスタ単位 (= 全戦略共通) の reconcile 設定。env 経由で main から渡す。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrategyAgentSettings {
    /// 戦略実行 MCP の URL。`spec.config.mcp.t-rader-strategy.url` に埋める。
    pub mcp_url: String,
    /// SSM パラメータ key の template。`{name}` を Agent 名で置換する。
    pub ssm_parameter_template: String,
    pub model: String,
    pub small_model: String,
}

impl StrategyAgentSettings {
    pub fn ssm_parameter_for(&self, agent_name: &str) -> String {
        self.ssm_parameter_template.replace("{name}", agent_name)
    }
}

/// Agent CR の所有関係を表す。子リソース (SA / ConfigMap / ExternalSecret) の
/// `metadata.ownerReferences` に埋め込み、Agent 削除で cascade GC される構造を作る。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentOwnerRef {
    pub name: String,
    pub uid: String,
}

fn owner_reference_json(owner: &AgentOwnerRef) -> Value {
    json!({
        "apiVersion": AGENT_API_VERSION,
        "kind": AGENT_KIND,
        "name": owner.name,
        "uid": owner.uid,
        "controller": true,
        "blockOwnerDeletion": true,
    })
}

fn owner_references(owner: Option<&AgentOwnerRef>) -> Value {
    match owner {
        Some(o) => json!([owner_reference_json(o)]),
        None => json!([]),
    }
}

fn agents_md_or_placeholder(spec: &StrategyAgentSpec) -> String {
    if spec.agents_md.trim().is_empty() {
        formatdoc! {"
            # Strategy {id}

            Placeholder. 戦略方針 / 制約 / KPI を記述する。
            ",
            id = spec.strategy_id,
        }
    } else {
        spec.agents_md.clone()
    }
}

pub fn build_agent_manifest(
    spec: &StrategyAgentSpec,
    settings: &StrategyAgentSettings,
    namespace: &str,
) -> Value {
    let mut mounts = vec![json!({
        "name": "workspace-context",
        "mountPath": "/workspace/AGENTS.md",
        "subPath": "AGENTS.md",
        "readOnly": true,
    })];
    for skill in spec.skills.keys() {
        mounts.push(json!({
            "name": "workspace-context",
            "mountPath": format!("/workspace/skills/{skill}.md"),
            "subPath": format!("skills__{skill}.md"),
            "readOnly": true,
        }));
    }

    json!({
        "apiVersion": AGENT_API_VERSION,
        "kind": AGENT_KIND,
        "metadata": {
            "name": spec.agent_name,
            "namespace": namespace,
        },
        "spec": {
            "workspaceDir": "/workspace",
            "serviceAccountName": spec.agent_name,
            "config": {
                "$schema": "https://opencode.ai/config.json",
                "model": settings.model,
                "small_model": settings.small_model,
                "share": "disabled",
                "autoupdate": false,
                "mcp": {
                    "t-rader-strategy": {
                        "type": "remote",
                        "url": settings.mcp_url,
                        "headers": {
                            "x-strategy-id": spec.strategy_id.to_string(),
                        },
                    },
                },
            },
            "credentials": [{
                "name": spec.agent_name,
                "secretRef": { "name": format!("{}-credentials", spec.agent_name) },
            }],
            "persistence": { "sessions": { "size": "1Gi" } },
            "standby": { "idleTimeout": "30m" },
            "podSpec": {
                "extraVolumes": [{
                    "name": "workspace-context",
                    "configMap": { "name": format!("{}-workspace", spec.agent_name) },
                }],
                "extraVolumeMounts": mounts,
            },
        },
    })
}

pub fn build_service_account_manifest(
    agent_name: &str,
    namespace: &str,
    owner: Option<&AgentOwnerRef>,
) -> Value {
    json!({
        "apiVersion": SA_API_VERSION,
        "kind": SA_KIND,
        "metadata": {
            "name": agent_name,
            "namespace": namespace,
            "ownerReferences": owner_references(owner),
        },
    })
}

pub fn build_workspace_configmap_manifest(
    spec: &StrategyAgentSpec,
    namespace: &str,
    owner: Option<&AgentOwnerRef>,
) -> Value {
    let mut data = serde_json::Map::new();
    data.insert(
        "AGENTS.md".to_string(),
        Value::String(agents_md_or_placeholder(spec)),
    );
    for (name, body) in &spec.skills {
        data.insert(format!("skills__{name}.md"), Value::String(body.clone()));
    }
    json!({
        "apiVersion": CONFIGMAP_API_VERSION,
        "kind": CONFIGMAP_KIND,
        "metadata": {
            "name": format!("{}-workspace", spec.agent_name),
            "namespace": namespace,
            "ownerReferences": owner_references(owner),
        },
        "data": data,
    })
}

pub fn build_external_secret_manifest(
    agent_name: &str,
    settings: &StrategyAgentSettings,
    namespace: &str,
    owner: Option<&AgentOwnerRef>,
) -> Value {
    let name = format!("{agent_name}-credentials");
    json!({
        "apiVersion": EXTERNAL_SECRET_API_VERSION,
        "kind": EXTERNAL_SECRET_KIND,
        "metadata": {
            "name": name,
            "namespace": namespace,
            "ownerReferences": owner_references(owner),
        },
        "spec": {
            "refreshInterval": "24h",
            "secretStoreRef": {
                "kind": "ClusterSecretStore",
                "name": "aws-parameter-store",
            },
            "target": {
                "name": name,
                "template": {
                    "type": "Opaque",
                    "data": {
                        "OPENCODE_API_KEY": "{{ .opencodeApiKey }}",
                    },
                },
            },
            "data": [{
                "secretKey": "opencodeApiKey",
                "remoteRef": {
                    "key": settings.ssm_parameter_for(agent_name),
                },
            }],
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::{fixture, rstest};

    #[rstest]
    fn agent_name_drops_dashes() {
        let id = Uuid::parse_str("12345678-1234-5678-1234-567812345678").unwrap();
        assert_eq!(
            agent_name_for(id),
            "strategy-12345678123456781234567812345678",
        );
    }

    #[fixture]
    fn settings() -> StrategyAgentSettings {
        StrategyAgentSettings {
            mcp_url: "http://t-rader-backend.t-rader/mcp/strategy".into(),
            ssm_parameter_template: DEFAULT_SSM_PARAMETER_TEMPLATE.into(),
            model: DEFAULT_AGENT_MODEL.into(),
            small_model: DEFAULT_AGENT_SMALL_MODEL.into(),
        }
    }

    #[fixture]
    fn spec() -> StrategyAgentSpec {
        let mut skills = BTreeMap::new();
        skills.insert("ja-stock".into(), "skill body\n".into());
        StrategyAgentSpec::new(
            Uuid::parse_str("12345678-1234-5678-1234-567812345678").unwrap(),
            "agents body\n",
            skills,
        )
    }

    #[rstest]
    fn agent_manifest_matches_expected(spec: StrategyAgentSpec, settings: StrategyAgentSettings) {
        assert_eq!(
            build_agent_manifest(&spec, &settings, "kubeopencode"),
            json!({
                "apiVersion": "kubeopencode.io/v1alpha1",
                "kind": "Agent",
                "metadata": {
                    "name": "strategy-12345678123456781234567812345678",
                    "namespace": "kubeopencode",
                },
                "spec": {
                    "workspaceDir": "/workspace",
                    "serviceAccountName": "strategy-12345678123456781234567812345678",
                    "config": {
                        "$schema": "https://opencode.ai/config.json",
                        "model": "opencode-go/minimax-m3",
                        "small_model": "opencode-go/deepseek-v4-flash",
                        "share": "disabled",
                        "autoupdate": false,
                        "mcp": {
                            "t-rader-strategy": {
                                "type": "remote",
                                "url": "http://t-rader-backend.t-rader/mcp/strategy",
                                "headers": {
                                    "x-strategy-id": "12345678-1234-5678-1234-567812345678",
                                },
                            },
                        },
                    },
                    "credentials": [{
                        "name": "strategy-12345678123456781234567812345678",
                        "secretRef": {
                            "name": "strategy-12345678123456781234567812345678-credentials",
                        },
                    }],
                    "persistence": { "sessions": { "size": "1Gi" } },
                    "standby": { "idleTimeout": "30m" },
                    "podSpec": {
                        "extraVolumes": [{
                            "name": "workspace-context",
                            "configMap": {
                                "name": "strategy-12345678123456781234567812345678-workspace",
                            },
                        }],
                        "extraVolumeMounts": [
                            {
                                "name": "workspace-context",
                                "mountPath": "/workspace/AGENTS.md",
                                "subPath": "AGENTS.md",
                                "readOnly": true,
                            },
                            {
                                "name": "workspace-context",
                                "mountPath": "/workspace/skills/ja-stock.md",
                                "subPath": "skills__ja-stock.md",
                                "readOnly": true,
                            },
                        ],
                    },
                },
            }),
        );
    }

    #[rstest]
    fn service_account_manifest_with_owner(spec: StrategyAgentSpec) {
        let owner = AgentOwnerRef {
            name: spec.agent_name.clone(),
            uid: "uid-1".into(),
        };
        assert_eq!(
            build_service_account_manifest(&spec.agent_name, "kubeopencode", Some(&owner)),
            json!({
                "apiVersion": "v1",
                "kind": "ServiceAccount",
                "metadata": {
                    "name": "strategy-12345678123456781234567812345678",
                    "namespace": "kubeopencode",
                    "ownerReferences": [{
                        "apiVersion": "kubeopencode.io/v1alpha1",
                        "kind": "Agent",
                        "name": "strategy-12345678123456781234567812345678",
                        "uid": "uid-1",
                        "controller": true,
                        "blockOwnerDeletion": true,
                    }],
                },
            }),
        );
    }

    #[rstest]
    fn configmap_manifest_with_agents_md_and_skills(spec: StrategyAgentSpec) {
        let owner = AgentOwnerRef {
            name: spec.agent_name.clone(),
            uid: "uid-1".into(),
        };
        assert_eq!(
            build_workspace_configmap_manifest(&spec, "kubeopencode", Some(&owner)),
            json!({
                "apiVersion": "v1",
                "kind": "ConfigMap",
                "metadata": {
                    "name": "strategy-12345678123456781234567812345678-workspace",
                    "namespace": "kubeopencode",
                    "ownerReferences": [{
                        "apiVersion": "kubeopencode.io/v1alpha1",
                        "kind": "Agent",
                        "name": "strategy-12345678123456781234567812345678",
                        "uid": "uid-1",
                        "controller": true,
                        "blockOwnerDeletion": true,
                    }],
                },
                "data": {
                    "AGENTS.md": "agents body\n",
                    "skills__ja-stock.md": "skill body\n",
                },
            }),
        );
    }

    #[rstest]
    fn configmap_uses_placeholder_when_agents_md_empty(settings: StrategyAgentSettings) {
        let _ = settings;
        let spec = StrategyAgentSpec::new(
            Uuid::parse_str("12345678-1234-5678-1234-567812345678").unwrap(),
            "",
            BTreeMap::new(),
        );
        let manifest = build_workspace_configmap_manifest(&spec, "kubeopencode", None);
        let expected = indoc::indoc! {"
            # Strategy 12345678-1234-5678-1234-567812345678

            Placeholder. 戦略方針 / 制約 / KPI を記述する。
            "};
        assert_eq!(manifest["data"]["AGENTS.md"], json!(expected));
    }

    #[rstest]
    fn external_secret_manifest(spec: StrategyAgentSpec, settings: StrategyAgentSettings) {
        assert_eq!(
            build_external_secret_manifest(&spec.agent_name, &settings, "kubeopencode", None),
            json!({
                "apiVersion": "external-secrets.io/v1",
                "kind": "ExternalSecret",
                "metadata": {
                    "name": "strategy-12345678123456781234567812345678-credentials",
                    "namespace": "kubeopencode",
                    "ownerReferences": [],
                },
                "spec": {
                    "refreshInterval": "24h",
                    "secretStoreRef": {
                        "kind": "ClusterSecretStore",
                        "name": "aws-parameter-store",
                    },
                    "target": {
                        "name": "strategy-12345678123456781234567812345678-credentials",
                        "template": {
                            "type": "Opaque",
                            "data": {
                                "OPENCODE_API_KEY": "{{ .opencodeApiKey }}",
                            },
                        },
                    },
                    "data": [{
                        "secretKey": "opencodeApiKey",
                        "remoteRef": {
                            "key": "/infra/kubeopencode/strategy-12345678123456781234567812345678-opencode-api-key",
                        },
                    }],
                },
            }),
        );
    }

    #[rstest]
    fn ssm_template_substitution() {
        let settings = StrategyAgentSettings {
            mcp_url: "x".into(),
            ssm_parameter_template: "/foo/{name}/bar".into(),
            model: "m".into(),
            small_model: "sm".into(),
        };
        assert_eq!(
            settings.ssm_parameter_for("strategy-abc"),
            "/foo/strategy-abc/bar",
        );
    }
}
