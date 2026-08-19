// backend は steps (jsonb) の中身を解釈せず素通しする契約のため、OpenAPI 生成型では
// `unknown` になる。実際のフィールド構成を定めるのは agent 側の StrategyTaskStepJson
// (agent/src/strategy-agent/agent-graph/step.ts) なので、型のみここから直接参照する。
import type { StrategyTaskStepJson as TaskStep } from 'agent/strategy-agent/agent-graph/step'
import { Result } from 'neverthrow'
import { parse as parseYaml } from 'yaml'

export type { TaskStep }

// フェーズが output で宣言する JSON Schema 相当のゆるい構造。中身の property 名
// (verdict 等) は戦略ごとの語彙なのでコードは解釈せず、`enum` の有無だけを見る。
export type AgentGraphOutputSchema = Record<string, unknown>

export interface AgentGraphPhaseSummary {
  key: string
  label: string
  model: string
  output?: AgentGraphOutputSchema
}

export type PhaseNode =
  | { kind: 'pending'; key: string; label: string; model: string }
  | {
      kind: 'single'
      key: string
      step: TaskStep
      outputSchema?: AgentGraphOutputSchema
    }
  | {
      kind: 'branch'
      key: string
      branches: TaskStep[]
      outputSchema?: AgentGraphOutputSchema
    }

function isRecord(v: unknown): v is Record<string, unknown> {
  return typeof v === 'object' && v !== null
}

// steps (jsonb) は backend が中身を解釈せず素通しするため unknown で届く。必須フィールドの
// 型だけを見て frontend 表示用の TaskStep として narrow する。
export function isTaskStep(v: unknown): v is TaskStep {
  return (
    isRecord(v) &&
    typeof v['phase_key'] === 'string' &&
    typeof v['label'] === 'string' &&
    typeof v['model'] === 'string' &&
    typeof v['status'] === 'string' &&
    typeof v['started_at'] === 'string' &&
    typeof v['trace_id'] === 'string' &&
    typeof v['span_id'] === 'string'
  )
}

const tryParseYaml = Result.fromThrowable((raw: string): unknown =>
  parseYaml(raw),
)

// isTaskStep を満たさない要素 (steps が空の初期状態など) は無視する。
export function readTaskSteps(steps: unknown): TaskStep[] {
  return Array.isArray(steps) ? steps.filter(isTaskStep) : []
}

// agent_graph の YAML から表示に必要な `key`/`label`/`model` だけを緩く取り出す。
// 保存時に backend 側で検証済みの内容を読むだけなので、他のフィールド (for_each 等)
// は解釈しない。パースに失敗した場合は「未設定」と同じ扱い (空配列) にする。
export function parseAgentGraphPhases(yaml: string): AgentGraphPhaseSummary[] {
  if (yaml.trim() === '') return []
  const parsed = tryParseYaml(yaml)
  if (parsed.isErr()) return []
  if (!isRecord(parsed.value)) return []
  const phases = parsed.value['phases']
  if (!Array.isArray(phases)) return []

  const result: AgentGraphPhaseSummary[] = []
  for (const p of phases) {
    if (!isRecord(p)) continue
    const { key, label, model, output } = p
    if (
      typeof key === 'string' &&
      typeof label === 'string' &&
      typeof model === 'string'
    ) {
      result.push({
        key,
        label,
        model,
        ...(isRecord(output) ? { output } : {}),
      })
    }
  }
  return result
}

// フラットな steps を phase_key でグルーピングし、agent_graph の設定順に木を組み直す。
// 設定に無いフェーズ (agent_graph 変更後の古いタスク等) は steps 内の初出順で末尾に追加する。
// steps に一件も無い設定フェーズは "pending" (待機) として表示する。
// steps が一件も無い (タスク未実行) ときは、configPhases があっても空配列を返す。
// これが無いと、実行済み/実行中のタスクと「まだ steps が一件も記録されていないタスク」を
// 区別できず、後者を恒久的に「全フェーズ待機」と誤表示してしまう。
export function buildPhaseNodes(
  configPhases: AgentGraphPhaseSummary[],
  steps: TaskStep[],
): PhaseNode[] {
  if (steps.length === 0) return []

  const sorted = [...steps].sort((a, b) =>
    a.started_at.localeCompare(b.started_at),
  )

  const stepsByPhase = new Map<string, TaskStep[]>()
  const seenOrder: string[] = []
  for (const step of sorted) {
    const existing = stepsByPhase.get(step.phase_key)
    if (existing == null) {
      stepsByPhase.set(step.phase_key, [step])
      seenOrder.push(step.phase_key)
    } else {
      existing.push(step)
    }
  }

  const configByKey = new Map(configPhases.map((p) => [p.key, p]))
  const keys = [
    ...configPhases.map((p) => p.key),
    ...seenOrder.filter((k) => !configByKey.has(k)),
  ]

  return keys.map((key): PhaseNode => {
    const phaseSteps = stepsByPhase.get(key) ?? []
    const first = phaseSteps[0]
    const config = configByKey.get(key)
    if (first == null) {
      return {
        kind: 'pending',
        key,
        label: config?.label ?? key,
        model: config?.model ?? '',
      }
    }
    const hasItem = phaseSteps.some((s) => s.item !== undefined)
    return hasItem
      ? {
          kind: 'branch',
          key,
          branches: phaseSteps,
          outputSchema: config?.output,
        }
      : { kind: 'single', key, step: first, outputSchema: config?.output }
  })
}

// output スキーマで enum 宣言された項目のうち、実際の output に値がある最初の 1 件を
// バッジとして返す。プロパティ名 (verdict 等) はコードから見て意味を持たない文字列で、
// 「enum で宣言されている」という構造だけを見て機械的に拾う。
export function findEnumBadge(
  outputSchema: AgentGraphOutputSchema | undefined,
  output: unknown,
): string | null {
  if (outputSchema == null || !isRecord(output)) return null
  for (const [key, def] of Object.entries(outputSchema)) {
    if (!isRecord(def) || !Array.isArray(def['enum'])) continue
    const value = output[key]
    if (typeof value === 'string') return value
  }
  return null
}

export function formatDuration(
  startedAt: string,
  finishedAt: string | null | undefined,
): string | null {
  if (finishedAt == null) return null
  const start = Date.parse(startedAt)
  const end = Date.parse(finishedAt)
  if (Number.isNaN(start) || Number.isNaN(end)) return null
  return `${((end - start) / 1000).toFixed(1)}s`
}

// トレースビューア (Tempo, Langfuse 等) の URL を組み立てる。`{trace_id}`/`{span_id}`
// プレースホルダを差し替えるだけの単純なテンプレート方式とすることで、ビューアの種類を
// 問わず backend の `TRACE_URL_TEMPLATE` (GET /api/config 経由) で環境ごとに指定できる
// ようにしている。未設定ならリンクを出さない。
export function buildTraceUrl(
  template: string | undefined,
  traceId: string,
  spanId: string | null | undefined,
): string | null {
  if (template == null || template === '') return null
  return template
    .replace('{trace_id}', encodeURIComponent(traceId))
    .replace('{span_id}', encodeURIComponent(spanId ?? ''))
}
