import { Button } from '@fohte/ui/button'
import { Link } from '@tanstack/react-router'
import { useState } from 'react'

import { type StatItem, StatRow } from '#components/portfolio/stat-row'
import {
  type AgentGraphOutputSchema,
  type AgentGraphPhaseSummary,
  type ExecutionPayloadReference,
  StepDetail,
  TaskExecutionTree,
  type TaskStep,
} from '#components/strategy-shell/task-execution-tree'
import {
  TaskRunNotes,
  type TaskRunNotesState,
} from '#components/strategy-shell/task-run-notes'
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
  notes: { id: string; title: string }[]
  notesState: TaskRunNotesState
  traceUrlTemplate?: string
  executionDefinition?: {
    status: 'loading' | 'error' | 'missing' | 'ready'
    digest?: string
    createdAt?: string
    definition?: unknown
  }
  executionDefinitionOpen?: boolean
  selectedPayload?: ExecutionPayloadReference | null
  payloadRead?:
    | { status: 'loading' | 'error' }
    | {
        status: 'ready'
        manifest: {
          status: string
          itemCount: number
          incompleteReason: string | null
        }
        page: {
          data: unknown
          totalCount: number
          nextCursor: number | null
        } | null
      }
  onSelectPayload?: (reference: ExecutionPayloadReference) => void
  onNextPayloadPage?: (cursor: number) => void
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
  notes,
  notesState,
  traceUrlTemplate,
  executionDefinition,
  executionDefinitionOpen = false,
  selectedPayload = null,
  payloadRead,
  onSelectPayload,
  onNextPayloadPage,
  taskLoadError = false,
}: TaskRunViewProps): React.ReactElement {
  const [selectedStep, setSelectedStep] = useState<{
    step: TaskStep
    outputSchema?: AgentGraphOutputSchema
    phaseDefinition?: Record<string, unknown>
  } | null>(null)

  return (
    <div className="space-y-4 font-sans text-foreground">
      <Link
        to="/strategies/$id/runs"
        params={{ id: strategyId }}
        className="inline-flex items-center gap-1 font-mono text-xs text-muted-foreground hover:text-foreground"
      >
        &lt; 実行一覧に戻る
      </Link>

      {task == null && taskLoadError ? (
        <p className="flex items-baseline gap-2 font-mono text-xs">
          <span className="uppercase tracking-wider text-destructive">
            error
          </span>
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
                  cls: task.phase === 'failed' ? 'text-destructive' : undefined,
                },
                {
                  label: 'ステップ',
                  value: `${String(steps.filter((s) => s.status === 'completed').length)}/${String(steps.length)}`,
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

          <TaskRunNotes notes={notes} state={notesState} />

          {executionDefinition != null && (
            <section
              aria-label="実行時の固定定義"
              className="border border-border bg-card p-4"
            >
              <h2 className="mb-2 font-mono text-xs font-bold uppercase tracking-wider">
                実行時の固定定義
              </h2>
              {executionDefinition.status === 'loading' ? (
                <p className="font-mono text-xs text-muted-foreground">
                  固定定義を読み込み中です。
                </p>
              ) : executionDefinition.status === 'error' ? (
                <p className="font-mono text-xs text-destructive">
                  固定定義を取得できませんでした。
                </p>
              ) : executionDefinition.status === 'missing' ? (
                <p className="font-mono text-xs text-muted-foreground">
                  固定定義の記録がありません。
                </p>
              ) : (
                <>
                  <p className="font-mono text-2xs text-muted-foreground">
                    SHA-256 {executionDefinition.digest} ·{' '}
                    {executionDefinition.createdAt}
                  </p>
                  <details open={executionDefinitionOpen} className="mt-2">
                    <summary className="cursor-pointer font-mono text-xs text-foreground">
                      定義 snapshot を表示
                    </summary>
                    <pre className="mt-2 overflow-x-auto whitespace-pre-wrap break-all font-mono text-2xs">
                      {JSON.stringify(executionDefinition.definition, null, 2)}
                    </pre>
                  </details>
                </>
              )}
            </section>
          )}

          <div className="grid grid-cols-1 gap-5 lg:grid-cols-(--grid-cols-task-run)">
            <div className="border border-border bg-card p-4">
              {steps.length === 0 ? (
                <p className="font-mono text-xs text-muted-foreground">
                  実行記録がありません。
                </p>
              ) : (
                <TaskExecutionTree
                  steps={steps}
                  configPhases={configPhases}
                  taskCompleted={task.phase === 'completed'}
                  traceUrlTemplate={traceUrlTemplate}
                  detailPlacement="external"
                  onSelectStep={setSelectedStep}
                />
              )}
            </div>
            <aside className="border border-border bg-card p-4">
              {selectedStep != null ? (
                <StepDetail
                  step={selectedStep.step}
                  outputSchema={selectedStep.outputSchema}
                  phaseDefinition={selectedStep.phaseDefinition}
                  traceUrlTemplate={traceUrlTemplate}
                  onSelectPayload={onSelectPayload}
                />
              ) : (
                <p className="font-mono text-xs text-muted-foreground">
                  ステップを選択してください。
                </p>
              )}
              {selectedPayload != null && (
                <ExecutionPayloadPanel
                  reference={selectedPayload}
                  payloadRead={payloadRead}
                  onNextPayloadPage={onNextPayloadPage}
                />
              )}
            </aside>
          </div>
        </>
      )}
    </div>
  )
}

function ExecutionPayloadPanel({
  reference,
  payloadRead,
  onNextPayloadPage,
}: {
  reference: ExecutionPayloadReference
  payloadRead: TaskRunViewProps['payloadRead']
  onNextPayloadPage?: (cursor: number) => void
}): React.ReactElement {
  const page = payloadRead?.status === 'ready' ? payloadRead.page : null
  const nextCursor = page?.nextCursor

  return (
    <section
      aria-label="実行 payload"
      className="mt-4 border-t border-border pt-4"
    >
      <h2 className="mb-2 font-mono text-xs font-bold">{reference.label}</h2>
      <p className="mb-2 break-all font-mono text-2xs text-muted-foreground">
        {reference.payloadId} · pointer {reference.pointer || '/'}
      </p>
      {payloadRead?.status === 'loading' ? (
        <p className="font-mono text-xs text-muted-foreground">
          payload を読み込み中です。
        </p>
      ) : payloadRead?.status === 'error' ? (
        <p className="font-mono text-xs text-destructive">
          payload を取得できませんでした。
        </p>
      ) : payloadRead?.status !== 'ready' ? (
        <p className="font-mono text-xs text-muted-foreground">
          payload を選択してください。
        </p>
      ) : payloadRead.manifest.status !== 'sealed' ? (
        <p className="font-mono text-xs text-muted-foreground">
          {payloadRead.manifest.status === 'staging'
            ? 'payload の記録が完了していません。'
            : 'payload は未完了のため内容を表示できません。'}{' '}
          {payloadRead.manifest.incompleteReason ?? ''}
        </p>
      ) : page == null ? (
        <p className="font-mono text-xs text-muted-foreground">
          payload page がありません。
        </p>
      ) : (
        <>
          <p className="mb-2 font-mono text-2xs text-muted-foreground">
            全 {page.totalCount} 件 · 表示{' '}
            {Array.isArray(page.data) ? page.data.length : 1} 件
          </p>
          <pre className="max-h-96 overflow-auto whitespace-pre-wrap break-all font-mono text-2xs">
            {JSON.stringify(page.data, null, 2)}
          </pre>
          {nextCursor != null && onNextPayloadPage != null && (
            <Button
              type="button"
              variant="link"
              size="sm"
              className="mt-2"
              onClick={() => {
                onNextPayloadPage(nextCursor)
              }}
            >
              次のページを表示
            </Button>
          )}
        </>
      )}
    </section>
  )
}
