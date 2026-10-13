import { Link } from '@tanstack/react-router'

import { StatusPill } from '#components/strategy-home/status-pill'
import { formatRelative } from '#lib/note-utils'

interface NoteLinkRowProps {
  noteId: string
  title: string
  status: string
  strategyName?: string
  author: string
  updatedAt: string
}

export function NoteLinkRow({
  noteId,
  title,
  status,
  strategyName,
  author,
  updatedAt,
}: NoteLinkRowProps) {
  return (
    <Link
      to="/notes/$noteId"
      params={{ noteId }}
      className="flex min-w-0 flex-col gap-1 px-3.5 py-2.5 hover:bg-surface-strong"
    >
      <span className="flex min-w-0 items-center gap-2">
        <span className="truncate text-sm text-foreground">{title}</span>
        {status === 'unread' && (
          <StatusPill status="unread" className="shrink-0" />
        )}
      </span>
      <span className="font-mono text-2xs text-muted-foreground">
        {strategyName != null && <>{strategyName} · </>}
        {author} · {formatRelative(updatedAt)}
      </span>
    </Link>
  )
}
