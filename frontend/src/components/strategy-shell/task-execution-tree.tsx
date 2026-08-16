// backend は steps (jsonb) の中身を解釈せず素通しする契約のため、OpenAPI 生成型では
// `unknown` になる。実際のフィールド構成を定めるのは agent 側の StrategyTaskStepJson
// (agent/src/strategy-agent/agent-graph/step.ts) なので、型のみここから直接参照する。
import type { StrategyTaskStepJson as TaskStep } from 'agent/strategy-agent/agent-graph/step'
import { Result } from 'neverthrow'
import { useEffect, useState } from 'react'
import { parse as parseYaml } from 'yaml'

import { cn } from '#lib/utils'

export type { TaskStep }

export interface AgentGraphPhaseSummary {
  key: string
  label: string
  model: string
}

export type PhaseNode =
  | { kind: 'pending'; key: string; label: string; model: string }
  | { kind: 'single'; key: string; step: TaskStep }
  | { kind: 'branch'; key: string; branches: TaskStep[] }

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
    const { key, label, model } = p
    if (
      typeof key === 'string' &&
      typeof label === 'string' &&
      typeof model === 'string'
    ) {
      result.push({ key, label, model })
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
    if (first == null) {
      const config = configByKey.get(key)
      return {
        kind: 'pending',
        key,
        label: config?.label ?? key,
        model: config?.model ?? '',
      }
    }
    const hasItem = phaseSteps.some((s) => s.item !== undefined)
    return hasItem
      ? { kind: 'branch', key, branches: phaseSteps }
      : { kind: 'single', key, step: first }
  })
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
// 問わず `.env.local` の VITE_TRACE_URL_TEMPLATE で環境ごとに指定できるようにしている。
// 未設定ならリンクを出さない。
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

export interface TaskExecutionTreeProps {
  steps: TaskStep[]
  configPhases: AgentGraphPhaseSummary[]
  /** トレースビューアの URL テンプレート (`{trace_id}`/`{span_id}` を差し替える)。未設定なら該当リンクを出さない */
  traceUrlTemplate?: string
  /**
   * 選択行の detail をどこに描画するか。`inline` (デフォルト) は選択行の直下に展開する
   * (フローティングチャットでの表示)。`external` は描画せず、選択状態のハイライトのみ行う
   * (2ペイン画面で右側の別パネルに `StepDetail` を描画するため)。
   */
  detailPlacement?: 'inline' | 'external'
  /** 選択行が変わるたびに呼ばれる (選択解除時は null) */
  onSelectStep?: (step: TaskStep | null) => void
}

const STATUS_LABEL: Record<TaskStep['status'], string> = {
  running: '実行中',
  completed: '完了',
  failed: '失敗',
}

interface RenderRow {
  key: string
  // グループ (phase_key) が切り替わる直前に「│」の継続行を挟む
  connectorBefore: boolean
  indent: boolean
  last: boolean
  content:
    | { kind: 'pending'; label: string; model: string }
    | { kind: 'step'; step: TaskStep }
}

function toRows(nodes: PhaseNode[]): RenderRow[] {
  const rows: RenderRow[] = []
  nodes.forEach((node, groupIndex) => {
    const connectorBefore = groupIndex > 0
    if (node.kind === 'pending') {
      rows.push({
        key: node.key,
        connectorBefore,
        indent: false,
        last: false,
        content: { kind: 'pending', label: node.label, model: node.model },
      })
    } else if (node.kind === 'single') {
      rows.push({
        key: node.key,
        connectorBefore,
        indent: false,
        last: false,
        content: { kind: 'step', step: node.step },
      })
    } else {
      node.branches.forEach((step, i) => {
        rows.push({
          key: `${node.key}:${String(i)}`,
          connectorBefore: connectorBefore && i === 0,
          indent: true,
          last: i === node.branches.length - 1,
          content: { kind: 'step', step },
        })
      })
    }
  })
  return rows
}

// タスクの実行状況 (`steps`) を agent_graph の設定順フェーズ木として描画する。
// `steps` が空 (agent_graph 未設定、または未実行) のときは何も描画しない。
export function TaskExecutionTree({
  steps,
  configPhases,
  traceUrlTemplate,
  detailPlacement = 'inline',
  onSelectStep,
}: TaskExecutionTreeProps): React.ReactElement | null {
  const [selectedKey, setSelectedKey] = useState<string | null>(null)

  const rows = toRows(buildPhaseNodes(configPhases, steps))

  // polling で選択中の step の中身 (status/output 等) が更新されても選択行 (key) は
  // 変わらないため、クリック時だけでなく steps 更新時にも最新の内容で呼び直す。
  // rows は render のたびに新しい参照になるので deps には steps/configPhases を使う。
  useEffect(() => {
    if (onSelectStep == null) return
    if (selectedKey == null) {
      onSelectStep(null)
      return
    }
    const row = rows.find((r) => r.key === selectedKey)
    onSelectStep(row?.content.kind === 'step' ? row.content.step : null)
  }, [steps, configPhases, selectedKey, onSelectStep])

  if (rows.length === 0) return null

  return (
    <div
      className="space-y-0 font-mono text-[12px]"
      data-testid="task-execution-tree"
    >
      {rows.map((row) => {
        const selected = row.content.kind === 'step' && selectedKey === row.key
        return (
          <div key={row.key}>
            {row.connectorBefore && (
              <div
                aria-hidden
                className="pl-[7px] text-[color:var(--color-text-tertiary)]"
              >
                │
              </div>
            )}
            <TreeRow
              row={row}
              selected={selected}
              onToggle={() => {
                setSelectedKey(selectedKey === row.key ? null : row.key)
              }}
            />
            {detailPlacement === 'inline' &&
              selected &&
              row.content.kind === 'step' && (
                <StepDetail
                  step={row.content.step}
                  traceUrlTemplate={traceUrlTemplate}
                />
              )}
          </div>
        )
      })}
    </div>
  )
}

function TreeRow({
  row,
  selected,
  onToggle,
}: {
  row: RenderRow
  selected: boolean
  onToggle: () => void
}): React.ReactElement {
  const prefix = row.indent ? (row.last ? '└─ ' : '├─ ') : ''

  if (row.content.kind === 'pending') {
    return (
      <div className="flex items-baseline gap-2 py-1 text-[color:var(--color-text-tertiary)]">
        <span>{prefix}○</span>
        <span className="flex-1 truncate">{row.content.label}</span>
        <span className="text-[10px]">{row.content.model}</span>
        <span className="text-[10px]">待機</span>
      </div>
    )
  }

  const { step } = row.content
  const label = step.item_label ?? step.label
  const duration = formatDuration(step.started_at, step.finished_at)

  return (
    <button
      type="button"
      onClick={onToggle}
      aria-expanded={selected}
      className={cn(
        'flex w-full items-baseline gap-2 py-1 text-left',
        selected
          ? 'text-[color:var(--color-text-primary)]'
          : 'text-[color:var(--color-text-secondary)] hover:text-[color:var(--color-text-primary)]',
      )}
    >
      <span
        className={cn(
          'text-[color:var(--color-accent-strategy)]',
          step.status === 'running' && 'animate-pulse',
        )}
      >
        {prefix}●
      </span>
      <span className="flex-1 truncate">{label}</span>
      <span className="text-[10px] text-[color:var(--color-text-tertiary)]">
        {step.model}
      </span>
      <span className="text-[10px] uppercase text-[color:var(--color-text-tertiary)]">
        {STATUS_LABEL[step.status]}
      </span>
      {duration != null && (
        <span className="text-[10px] text-[color:var(--color-text-tertiary)]">
          {duration}
        </span>
      )}
    </button>
  )
}

export function StepDetail({
  step,
  traceUrlTemplate,
}: {
  step: TaskStep
  traceUrlTemplate?: string
}): React.ReactElement {
  const traceUrl = buildTraceUrl(traceUrlTemplate, step.trace_id, step.span_id)

  return (
    <div className="mb-2 ml-4 space-y-2 border-l border-[color:var(--color-border-strategy)] py-1 pl-3 text-[11px] text-[color:var(--color-text-secondary)]">
      {step.item !== undefined && <JsonBlock label="input" value={step.item} />}
      {step.output !== undefined && (
        <JsonBlock label="output" value={step.output} />
      )}
      {step.status === 'failed' && step.error != null && step.error !== '' && (
        <pre className="whitespace-pre-wrap">{step.error}</pre>
      )}
      {traceUrl != null && (
        <a
          href={traceUrl}
          target="_blank"
          rel="noreferrer"
          className="block text-[color:var(--color-accent-strategy)] hover:underline"
        >
          → トレースを開く
        </a>
      )}
    </div>
  )
}

function JsonBlock({
  label,
  value,
}: {
  label: string
  value: unknown
}): React.ReactElement {
  return (
    <div>
      <div className="mb-1 text-[10px] uppercase tracking-wider text-[color:var(--color-text-tertiary)]">
        {label}
      </div>
      <pre className="overflow-x-auto whitespace-pre-wrap break-all">
        {JSON.stringify(value, null, 2)}
      </pre>
    </div>
  )
}
