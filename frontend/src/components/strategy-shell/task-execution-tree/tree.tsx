import { useEffect, useState } from 'react'

import {
  type AgentGraphOutputSchema,
  type AgentGraphPhaseSummary,
  buildPhaseNodes,
  findEnumBadge,
  formatDuration,
  type PhaseNode,
  stepSubtitle,
  type TaskStep,
} from '#components/strategy-shell/task-execution-tree/model'
import { StepDetail } from '#components/strategy-shell/task-execution-tree/step-detail'
import { cn } from '#lib/utils'

export interface TaskExecutionTreeProps {
  steps: TaskStep[]
  configPhases: AgentGraphPhaseSummary[]
  /** ノートへのリンク組み立てに使う戦略 id */
  strategyId: string
  /** トレースビューアの URL テンプレート (`{trace_id}`/`{span_id}` を差し替える)。未設定なら該当リンクを出さない */
  traceUrlTemplate?: string
  /**
   * 選択行の detail をどこに描画するか。`inline` (デフォルト) は選択行の直下に展開する
   * (フローティングチャットでの表示)。`external` は描画せず、選択状態のハイライトのみ行う
   * (2ペイン画面で右側の別パネルに `StepDetail` を描画するため)。
   */
  detailPlacement?: 'inline' | 'external'
  /** 選択行が変わるたびに呼ばれる (選択解除時は null) */
  onSelectStep?: (
    selection: { step: TaskStep; outputSchema?: AgentGraphOutputSchema } | null,
  ) => void
}

const STATUS_LABEL: Record<TaskStep['status'], string> = {
  running: '実行中',
  completed: '完了',
  failed: '失敗',
}

// 色分けは step.status のみで決める。enum の値 (rejected 等) で分岐すると、
// UI が戦略の語彙 (何が「悪い」結果か) を知ることになってしまうため禁止。
const STATUS_COLOR: Record<TaskStep['status'], string> = {
  running: 'text-[color:var(--color-status-task-running)]',
  completed: 'text-[color:var(--color-text-secondary)]',
  failed: 'text-[color:var(--color-accent-strategy)]',
}

interface RenderRow {
  key: string
  // グループ (phase_key) が切り替わる直前に「│」の継続行を挟む
  connectorBefore: boolean
  indent: boolean
  last: boolean
  content:
    | { kind: 'pending'; label: string; model: string }
    | { kind: 'step'; step: TaskStep; outputSchema?: AgentGraphOutputSchema }
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
        content: {
          kind: 'step',
          step: node.step,
          outputSchema: node.outputSchema,
        },
      })
    } else {
      node.branches.forEach((step, i) => {
        rows.push({
          key: `${node.key}:${String(i)}`,
          connectorBefore: connectorBefore && i === 0,
          indent: true,
          last: i === node.branches.length - 1,
          content: { kind: 'step', step, outputSchema: node.outputSchema },
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
  strategyId,
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
    onSelectStep(
      row?.content.kind === 'step'
        ? { step: row.content.step, outputSchema: row.content.outputSchema }
        : null,
    )
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
                  strategyId={strategyId}
                  step={row.content.step}
                  outputSchema={row.content.outputSchema}
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

  const { step, outputSchema } = row.content
  const label = step.item_label ?? step.label
  const duration = formatDuration(step.started_at, step.finished_at)
  const badgeText =
    findEnumBadge(outputSchema, step.output) ?? STATUS_LABEL[step.status]
  const subtitle = stepSubtitle(step)

  return (
    <button
      type="button"
      onClick={onToggle}
      aria-expanded={selected}
      className={cn(
        'flex w-full flex-col gap-0.5 py-1 text-left',
        selected
          ? 'text-[color:var(--color-text-primary)]'
          : 'text-[color:var(--color-text-secondary)] hover:text-[color:var(--color-text-primary)]',
      )}
    >
      <span className="flex items-baseline gap-2">
        <span
          className={cn(
            STATUS_COLOR[step.status],
            step.status === 'running' && 'animate-pulse',
          )}
        >
          {prefix}●
        </span>
        <span className="flex-1 truncate">{label}</span>
        <span className="text-[10px] text-[color:var(--color-text-tertiary)]">
          {step.model}
        </span>
        <span className={cn('text-[10px]', STATUS_COLOR[step.status])}>
          {badgeText}
        </span>
        {duration != null && (
          <span className="text-[10px] text-[color:var(--color-text-tertiary)]">
            {duration}
          </span>
        )}
      </span>
      {subtitle != null && (
        <span
          data-testid="step-subtitle"
          className="truncate pl-[18px] text-[10px] text-[color:var(--color-text-tertiary)]"
        >
          {subtitle}
        </span>
      )}
    </button>
  )
}
