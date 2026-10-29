import { Link } from '@tanstack/react-router'

import { ModelCallLimitBadge } from '#components/model-call-limit-badge'
import { sourceLabel } from '#components/strategy-shell/task-run-view'
import { taskPhaseLabel } from '#components/task-phase-label'
import { Skeleton } from '#components/ui/skeleton'
import { formatRelative } from '#lib/note-utils'

interface TaskRunListItem {
  taskId: string
  strategyId: string
  strategyName?: string
  prompt: string
  source: string
  phase: string
  purpose: string | null
  createdAt: string
  modelCallLimitReached: boolean
}

export interface TaskRunListViewProps {
  tasks: TaskRunListItem[] | null
}

function PhaseBadge({ phase }: { phase: string }) {
  return (
    <span
      className={
        phase === 'failed'
          ? 'font-mono text-2xs uppercase tracking-wider text-destructive'
          : 'font-mono text-2xs uppercase tracking-wider text-muted-foreground'
      }
    >
      {taskPhaseLabel(phase)}
    </span>
  )
}

export function TaskRunListView({
  tasks,
}: TaskRunListViewProps): React.ReactElement {
  if (tasks == null) {
    return (
      <div className="space-y-2">
        <Skeleton className="h-14 w-full" />
        <Skeleton className="h-14 w-full" />
        <Skeleton className="h-14 w-full" />
      </div>
    )
  }

  if (tasks.length === 0) {
    return (
      <p className="font-mono text-xs text-muted-foreground">
        過去のタスクはまだありません。
      </p>
    )
  }

  return (
    <ul className="space-y-2">
      {tasks.map((t) => (
        <li key={t.taskId}>
          <Link
            to="/strategies/$id/runs/$taskId"
            params={{ id: t.strategyId, taskId: t.taskId }}
            className="flex items-center gap-3 border border-border bg-card px-3.5 py-2.5 hover:border-muted-foreground"
          >
            <span className="flex-1 truncate text-sm text-foreground">
              {t.prompt}
            </span>
            {t.strategyName != null && (
              <span className="font-mono text-2xs text-muted-foreground">
                {t.strategyName}
              </span>
            )}
            {t.purpose != null && (
              <span className="font-mono text-2xs text-muted-foreground">
                {t.purpose}
              </span>
            )}
            <span className="font-mono text-2xs text-muted-foreground">
              {sourceLabel(t.source)}
            </span>
            <PhaseBadge phase={t.phase} />
            {t.modelCallLimitReached && <ModelCallLimitBadge />}
            <span className="font-mono text-2xs text-muted-foreground">
              {formatRelative(t.createdAt)}
            </span>
          </Link>
        </li>
      ))}
    </ul>
  )
}
