import { Link } from '@tanstack/react-router'
import { useState } from 'react'

import { type StatItem, StatRow } from '#components/portfolio/stat-row'
import {
  type AgentGraphPhaseSummary,
  StepDetail,
  TaskExecutionTree,
  type TaskStep,
} from '#components/strategy-shell/task-execution-tree'
import { Skeleton } from '#components/ui/skeleton'

export interface TaskRunViewProps {
  strategyId: string
  task: {
    taskId: string
    prompt: string
    source: string
    phase: string
    createdAt: string
    updatedAt: string
    errorSummary: string | null
  } | null
  steps: TaskStep[]
  configPhases: AgentGraphPhaseSummary[]
  generatedNotesCount: number
  traceUrlTemplate?: string
  /** task の取得に失敗したか (`task` が null の間、ロード中との表示を出し分けるために使う) */
  taskLoadError?: boolean
}

// source は backend が定義する投入経路の固定値
const SOURCE_LABELS: Record<string, string> = {
  'mgmt-mcp': '管理MCP',
  frontend: 'フローティングチャット',
  cron: 'cron trigger',
  hook: 'hook trigger',
  review: 'レビュー',
}

export function sourceLabel(source: string): string {
  return SOURCE_LABELS[source] ?? source
}

// Intl.DateTimeFormat の生成は重いのでモジュールスコープで 1 度だけ確保する
const DATE_FORMATTER = new Intl.DateTimeFormat('ja-JP', {
  month: '2-digit',
  day: '2-digit',
  hour: '2-digit',
  minute: '2-digit',
})

function formatDateTime(iso: string): string {
  const d = new Date(iso)
  return Number.isNaN(d.getTime()) ? iso : DATE_FORMATTER.format(d)
}

// 秒未満は四捨五入、分オーダーになりうるため `formatDuration` (秒のみ, 小数第1位まで)
// とは別に軽量なフォーマッタを用意する。
function formatElapsed(ms: number): string {
  const totalSec = Math.max(0, Math.round(ms / 1000))
  const min = Math.floor(totalSec / 60)
  const sec = totalSec % 60
  return min > 0 ? `${String(min)}m${String(sec)}s` : `${String(sec)}s`
}

function computeElapsed(task: NonNullable<TaskRunViewProps['task']>): string {
  const start = Date.parse(task.createdAt)
  const end =
    task.phase === 'pending' || task.phase === 'running'
      ? Date.now()
      : Date.parse(task.updatedAt)
  if (Number.isNaN(start) || Number.isNaN(end)) return '—'
  return formatElapsed(end - start)
}

export function TaskRunView({
  strategyId,
  task,
  steps,
  configPhases,
  generatedNotesCount,
  traceUrlTemplate,
  taskLoadError = false,
}: TaskRunViewProps): React.ReactElement {
  const [selectedStep, setSelectedStep] = useState<TaskStep | null>(null)

  return (
    <div className="space-y-4 font-sans text-[color:var(--color-text-primary)]">
      <Link
        to="/strategies/$id/runs"
        params={{ id: strategyId }}
        className="inline-flex items-center gap-1 font-mono text-[12px] text-[color:var(--color-text-tertiary)] hover:text-[color:var(--color-accent-strategy)]"
      >
        &lt; 実行一覧に戻る
      </Link>

      {task == null && taskLoadError ? (
        <p className="flex items-baseline gap-2 font-mono text-[12px]">
          <span className="uppercase tracking-wider text-[color:var(--color-accent-strategy)]">
            error
          </span>
          <span className="text-[color:var(--color-text-secondary)]">
            タスクの取得に失敗しました。
          </span>
        </p>
      ) : task == null ? (
        <div className="space-y-4">
          <Skeleton className="h-8 w-2/3" />
          <Skeleton className="h-5 w-full max-w-[480px]" />
          <Skeleton className="h-[88px] w-full" />
          <Skeleton className="h-[320px] w-full" />
        </div>
      ) : (
        <>
          <header>
            <h1 className="mb-1.5 text-[22px] font-bold leading-tight tracking-tight">
              {task.prompt}
            </h1>
            <p className="font-mono text-[11px] text-[color:var(--color-text-tertiary)]">
              {sourceLabel(task.source)} から投入 ·{' '}
              {formatDateTime(task.createdAt)} · 経過 {computeElapsed(task)}
            </p>
          </header>

          <StatRow
            stats={
              [
                {
                  label: 'PHASE',
                  value: task.phase.toUpperCase(),
                  cls:
                    task.phase === 'failed'
                      ? 'text-[color:var(--color-accent-strategy)]'
                      : undefined,
                },
                {
                  label: 'ステップ',
                  value: `${String(steps.filter((s) => s.status === 'completed').length)}/${String(steps.length)}`,
                },
                {
                  label: '生成ノート',
                  value: generatedNotesCount.toLocaleString(),
                },
              ] satisfies StatItem[]
            }
          />

          {task.phase === 'failed' &&
            task.errorSummary != null &&
            task.errorSummary !== '' && (
              <pre className="whitespace-pre-wrap border border-[color:var(--color-border-strategy)] bg-[color:var(--color-bg-primary)] p-2 font-mono text-[11px] text-[color:var(--color-text-secondary)]">
                {task.errorSummary}
              </pre>
            )}

          <div className="grid grid-cols-1 gap-5 lg:grid-cols-[minmax(0,1fr)_360px]">
            <div className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)] p-4">
              {steps.length === 0 ? (
                <p className="font-mono text-[12px] text-[color:var(--color-text-tertiary)]">
                  実行記録がありません。
                </p>
              ) : (
                <TaskExecutionTree
                  steps={steps}
                  configPhases={configPhases}
                  traceUrlTemplate={traceUrlTemplate}
                  detailPlacement="external"
                  onSelectStep={setSelectedStep}
                />
              )}
            </div>
            <aside className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)] p-4">
              {selectedStep != null ? (
                <StepDetail
                  step={selectedStep}
                  traceUrlTemplate={traceUrlTemplate}
                />
              ) : (
                <p className="font-mono text-[12px] text-[color:var(--color-text-tertiary)]">
                  ステップを選択してください。
                </p>
              )}
            </aside>
          </div>
        </>
      )}
    </div>
  )
}
