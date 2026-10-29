import { Link } from '@tanstack/react-router'

import { ModelCallLimitBadge } from '#components/model-call-limit-badge'
import { taskPhaseLabel } from '#components/task-phase-label'
import { Skeleton } from '#components/ui/skeleton'
import type { components } from '#lib/api/schema.gen'
import { formatRelative } from '#lib/note-utils'

type Strategy = components['schemas']['Strategy']
type Task = components['schemas']['StrategyTaskSummary']

export function DashboardRecentRuns({
  tasks,
  strategies,
  isError,
}: {
  tasks: Task[] | null
  strategies: Strategy[]
  isError: boolean
}) {
  const recentTasks =
    tasks == null
      ? []
      : [...tasks]
          .sort((left, right) =>
            right.created_at.localeCompare(left.created_at),
          )
          .slice(0, 3)
  const strategyNameById = new Map(
    strategies.map((strategy) => [strategy.id, strategy.name]),
  )

  return (
    <section className="space-y-2" aria-labelledby="dashboard-runs-title">
      <div className="flex items-baseline justify-between gap-3">
        <h2
          id="dashboard-runs-title"
          className="font-mono text-2xs uppercase tracking-wider text-muted-foreground"
        >
          <span className="mr-2 text-primary">&gt;</span>
          直近の実行
        </h2>
        <Link
          to="/runs"
          className="shrink-0 font-mono text-2xs text-primary hover:underline"
        >
          実行履歴 →
        </Link>
      </div>
      <div className="divide-y divide-border border border-border bg-card">
        {isError ? (
          <p className="px-4 py-5 font-mono text-xs text-muted-foreground">
            実行履歴を読み込めませんでした。
          </p>
        ) : tasks == null ? (
          <div className="space-y-2 p-3">
            <Skeleton className="h-10 w-full" />
            <Skeleton className="h-10 w-full" />
          </div>
        ) : recentTasks.length === 0 ? (
          <p className="px-4 py-7 text-center font-mono text-xs text-muted-foreground">
            実行履歴はまだありません。
          </p>
        ) : (
          recentTasks.map((task) => (
            <Link
              key={task.task_id}
              to="/strategies/$id/runs/$taskId"
              params={{ id: task.strategy_id, taskId: task.task_id }}
              className="flex min-w-0 items-center gap-2.5 px-3.5 py-2.5 hover:bg-surface-strong"
            >
              <span className="min-w-0 flex-1 truncate text-sm text-foreground">
                {task.prompt}
              </span>
              <span className="hidden max-w-24 truncate font-mono text-2xs text-muted-foreground sm:inline">
                {strategyNameById.get(task.strategy_id) ?? '戦略'}
              </span>
              <span
                className={`shrink-0 font-mono text-2xs uppercase tracking-wider ${
                  task.phase === 'failed'
                    ? 'text-primary'
                    : task.phase === 'running'
                      ? 'text-up'
                      : 'text-muted-foreground'
                }`}
              >
                {taskPhaseLabel(task.phase)}
              </span>
              {task.model_call_limit_reached && <ModelCallLimitBadge />}
              <span className="hidden shrink-0 font-mono text-2xs text-muted-foreground sm:inline">
                {formatRelative(task.created_at)}
              </span>
            </Link>
          ))
        )}
      </div>
    </section>
  )
}
