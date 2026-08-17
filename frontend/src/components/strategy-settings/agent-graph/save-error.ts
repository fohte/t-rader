// ponytail: backend の thiserror 文言 (backend/src/services/agent_graph.rs の
// `AgentGraphError` 各 `#[error(...)]`) にパターンマッチしている。文言が変わると phase
// 特定は静かに壊れる (エラー自体は消えず、カードへの紐付けだけが外れる)。直すなら backend
// の 400 レスポンスに phase_key を構造化フィールドとして載せる。
const PHASE_KEY_PATTERN = /^phase(?: key)? "([^"]+)"/

/**
 * `PUT /api/strategies/{id}/agent-graph` が返すエラーメッセージから、原因になった
 * フェーズの key を抜き出す。`AgentGraphError::InvalidYaml` のようにフェーズを
 * 特定できないエラーでは null を返す (呼び出し側は全体バナーのみ表示する)。
 */
export function extractPhaseKeyFromSaveError(message: string): string | null {
  return PHASE_KEY_PATTERN.exec(message)?.[1] ?? null
}
