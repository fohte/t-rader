import { Link } from '@tanstack/react-router'

import { sourceLabel } from '#components/strategy-shell/task-run-view'
import { Skeleton } from '#components/ui/skeleton'
import { formatRelative } from '#lib/note-utils'

export interface TaskRunListItem {
  taskId: string
  prompt: string
  source: string
  phase: string
  createdAt: string
}

export interface TaskRunListViewProps {
  strategyId: string
  tasks: TaskRunListItem[] | null
}

const PHASE_LABEL: Record<string, string> = {
  pending: 'PENDING',
  running: 'RUNNING',
  completed: 'COMPLETED',
  failed: 'FAILED',
}

function PhaseBadge({ phase }: { phase: string }) {
  return (
    <span
      className={
        phase === 'failed'
          ? 'font-mono text-2xs uppercase tracking-wider text-primary'
          : 'font-mono text-2xs uppercase tracking-wider text-muted-foreground'
      }
    >
      {PHASE_LABEL[phase] ?? phase}
    </span>
  )
}

export function TaskRunListView({
  strategyId,
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
            params={{ id: strategyId, taskId: t.taskId }}
            className="flex items-center gap-3 border border-border bg-card px-3.5 py-2.5 hover:border-primary"
          >
            <span className="flex-1 truncate text-sm text-foreground">
              {t.prompt}
            </span>
            <span className="font-mono text-2xs text-muted-foreground">
              {sourceLabel(t.source)}
            </span>
            <PhaseBadge phase={t.phase} />
            <span className="font-mono text-2xs text-muted-foreground">
              {formatRelative(t.createdAt)}
            </span>
          </Link>
        </li>
      ))}
    </ul>
  )
}
