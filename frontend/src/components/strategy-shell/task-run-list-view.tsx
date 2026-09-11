import { Link } from '@tanstack/react-router'

import { sourceLabel } from '#components/strategy-shell/task-run-view'
import { Skeleton } from '#components/ui/skeleton'
import { formatRelative } from '#lib/note-utils'

export interface TaskRunListItem {
  taskId: string
  strategyId: string
  /** 口座横断ページなど、どの戦略のタスクか明示したい文脈でのみ渡す */
  strategyName?: string
  prompt: string
  source: string
  phase: string
  purpose: string | null
  createdAt: string
}

export interface TaskRunListViewProps {
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
            className="flex items-center gap-3 border border-border bg-card px-3.5 py-2.5 hover:border-primary"
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
            <span className="font-mono text-2xs text-muted-foreground">
              {formatRelative(t.createdAt)}
            </span>
          </Link>
        </li>
      ))}
    </ul>
  )
}
