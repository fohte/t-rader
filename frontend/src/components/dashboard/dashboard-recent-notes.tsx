import { Chip } from '@fohte/ui/chip'
import { Link } from '@tanstack/react-router'

import { StatusPill } from '#components/strategy-home/status-pill'
import { Skeleton } from '#components/ui/skeleton'
import type { components } from '#lib/api/schema.gen'
import { formatRelative } from '#lib/note-utils'

type NoteOverview = components['schemas']['NoteOverviewResponse']

export function DashboardRecentNotes({
  notes,
  isError,
}: {
  notes: NoteOverview[] | null
  isError: boolean
}) {
  const recentNotes =
    notes == null
      ? []
      : [...notes]
          .sort((left, right) =>
            right.representative.updated_at.localeCompare(
              left.representative.updated_at,
            ),
          )
          .slice(0, 3)

  return (
    <section className="space-y-2" aria-labelledby="dashboard-notes-title">
      <div className="flex items-baseline justify-between gap-3">
        <h2
          id="dashboard-notes-title"
          className="font-mono text-2xs uppercase tracking-wider text-muted-foreground"
        >
          <span className="mr-2 text-primary">&gt;</span>
          最近のノート
        </h2>
        <Link
          to="/notes"
          className="shrink-0 font-mono text-2xs text-primary hover:underline"
        >
          ノート一覧 →
        </Link>
      </div>
      <div className="divide-y divide-border border border-border bg-card">
        {isError ? (
          <p className="px-4 py-5 font-mono text-xs text-muted-foreground">
            ノートを読み込めませんでした。
          </p>
        ) : notes == null ? (
          <div className="space-y-2 p-3">
            <Skeleton className="h-10 w-full" />
            <Skeleton className="h-10 w-full" />
          </div>
        ) : recentNotes.length === 0 ? (
          <p className="px-4 py-7 text-center font-mono text-xs text-muted-foreground">
            ノートはまだありません。
          </p>
        ) : (
          recentNotes.map((overview) => {
            const note = overview.representative

            return (
              <Link
                key={note.id}
                to="/notes/$noteId"
                params={{ noteId: note.id }}
                className="flex min-w-0 flex-col gap-1 px-3.5 py-2.5 hover:bg-surface-strong"
              >
                <span className="truncate text-sm text-foreground">
                  {note.title}
                </span>
                <span className="font-mono text-2xs text-muted-foreground">
                  {note.created_by_kind === 'human' ? '自分' : 'エージェント'} ·{' '}
                  {formatRelative(note.updated_at)}
                </span>
                <span className="flex flex-wrap items-center gap-2 font-mono text-2xs">
                  <StatusPill status={note.status} />
                  {!note.is_current && overview.current == null && (
                    <Chip>現行なし</Chip>
                  )}
                  {overview.unread_version_count > 0 && (
                    <Chip>
                      未読 {String(overview.unread_version_count)} バージョン
                    </Chip>
                  )}
                </span>
              </Link>
            )
          })
        )}
      </div>
    </section>
  )
}
