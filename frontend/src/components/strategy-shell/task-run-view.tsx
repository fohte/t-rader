import { Link } from '@tanstack/react-router'
import { useState } from 'react'

import { type StatItem, StatRow } from '#components/portfolio/stat-row'
import {
  type AgentGraphOutputSchema,
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
export function formatElapsed(ms: number): string {
  const totalSec = Math.max(0, Math.round(ms / 1000))
  const min = Math.floor(totalSec / 60)
  const sec = totalSec % 60
  return min > 0 ? `${String(min)}m${String(sec)}s` : `${String(sec)}s`
}

export function computeElapsed(
  task: NonNullable<TaskRunViewProps['task']>,
): string {
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
  const [selectedStep, setSelectedStep] = useState<{
    step: TaskStep
    outputSchema?: AgentGraphOutputSchema
  } | null>(null)

  return (
    <div className="space-y-4 font-sans text-foreground">
      <Link
        to="/strategies/$id/runs"
        params={{ id: strategyId }}
        className="inline-flex items-center gap-1 font-mono text-xs text-muted-foreground hover:text-primary"
      >
        &lt; 実行一覧に戻る
      </Link>

      {task == null && taskLoadError ? (
        <p className="flex items-baseline gap-2 font-mono text-xs">
          <span className="uppercase tracking-wider text-primary">error</span>
          <span className="text-muted-foreground-strong">
            タスクの取得に失敗しました。
          </span>
        </p>
      ) : task == null ? (
        <div className="space-y-4">
          <Skeleton className="h-8 w-2/3" />
          <Skeleton className="h-5 w-full max-w-120" />
          <Skeleton className="h-22 w-full" />
          <Skeleton className="h-80 w-full" />
        </div>
      ) : (
        <>
          <header>
            <h1 className="mb-1.5 text-2xl font-bold leading-tight tracking-tight">
              {task.prompt}
            </h1>
            <p className="font-mono text-2xs text-muted-foreground">
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
                  cls: task.phase === 'failed' ? 'text-primary' : undefined,
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
              <pre className="whitespace-pre-wrap border border-border bg-background p-2 font-mono text-2xs text-muted-foreground-strong">
                {task.errorSummary}
              </pre>
            )}

          <div
            // grid-cols-[minmax(0,1fr)_360px] は可変幅+固定幅の grid-template-columns で、対応する非 arbitrary な scale utility が存在しない
            className="grid grid-cols-1 gap-5 lg:grid-cols-[minmax(0,1fr)_360px]"
          >
            <div className="border border-border bg-card p-4">
              {steps.length === 0 ? (
                <p className="font-mono text-xs text-muted-foreground">
                  実行記録がありません。
                </p>
              ) : (
                <TaskExecutionTree
                  steps={steps}
                  configPhases={configPhases}
                  strategyId={strategyId}
                  traceUrlTemplate={traceUrlTemplate}
                  detailPlacement="external"
                  onSelectStep={setSelectedStep}
                />
              )}
            </div>
            <aside className="border border-border bg-card p-4">
              {selectedStep != null ? (
                <StepDetail
                  strategyId={strategyId}
                  step={selectedStep.step}
                  outputSchema={selectedStep.outputSchema}
                  traceUrlTemplate={traceUrlTemplate}
                />
              ) : (
                <p className="font-mono text-xs text-muted-foreground">
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
