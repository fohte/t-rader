import { Link } from '@tanstack/react-router'

import { StatusPill } from '#components/strategy-home/status-pill'
import type { NoteVersion } from '#lib/api/note-version-types'
import { formatRelative } from '#lib/note-utils'

export interface PendingNoteVersionsViewProps {
  versions: NoteVersion[]
  isPending: boolean
  hasError: boolean
}

export function PendingNoteVersionsView({
  versions,
  isPending,
  hasError,
}: PendingNoteVersionsViewProps) {
  const orderedVersions = [...versions].sort(
    (left, right) =>
      Date.parse(right.created_at) - Date.parse(left.created_at) ||
      right.version_no - left.version_no,
  )

  return (
    <section className="border border-border bg-card">
      <header className="border-b border-border px-4 py-3">
        <h1 className="text-xl font-bold tracking-tight text-foreground">
          承認待ちバージョン
        </h1>
        <p className="mt-1 font-mono text-xs text-muted-foreground">
          未レビューのノートバージョンをまとめて確認できます。
        </p>
      </header>
      {isPending ? (
        <p className="px-4 py-4 font-mono text-xs text-muted-foreground">
          読み込み中…
        </p>
      ) : hasError ? (
        <p className="px-4 py-4 font-mono text-xs text-primary">
          承認待ちバージョンを読み込めませんでした。
        </p>
      ) : orderedVersions.length === 0 ? (
        <p className="px-4 py-4 font-mono text-xs text-muted-foreground">
          承認待ちのバージョンはありません。
        </p>
      ) : (
        <div className="divide-y divide-border">
          {orderedVersions.map((version) => (
            <Link
              key={version.id}
              to="/notes/$noteId"
              params={{ noteId: version.note_id }}
              search={{ version: version.version_no }}
              className="flex flex-col gap-2 px-4 py-3 hover:bg-surface-strong"
            >
              <span className="flex flex-wrap items-center justify-between gap-2">
                <span className="font-mono text-xs text-muted-foreground">
                  ノート {version.note_id} · v{String(version.version_no)}
                </span>
                <StatusPill status={version.status} />
              </span>
              <span className="text-base font-semibold text-foreground">
                {version.title}
              </span>
              {version.change_reason != null &&
                version.change_reason !== '' && (
                  <span className="line-clamp-2 text-sm text-muted-foreground-strong">
                    {version.change_reason}
                  </span>
                )}
              <span className="font-mono text-2xs text-muted-foreground">
                {version.created_by_kind === 'llm' ? 'analyst' : 'ユーザー'} ·{' '}
                {formatRelative(version.created_at)}
              </span>
            </Link>
          ))}
        </div>
      )}
    </section>
  )
}
