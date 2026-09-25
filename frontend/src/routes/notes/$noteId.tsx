import { createFileRoute, Link } from '@tanstack/react-router'

import { HistoryPanel } from '#components/note-detail/history-panel'
import { NoteHypothesesPanel } from '#components/note-detail/note-hypotheses-panel'
import { NoteVersionChatAction } from '#components/note-detail/note-version-chat-action'
import { NoteVersionDiffPanel } from '#components/note-detail/note-version-diff-panel'
import { NoteVersionFallback } from '#components/note-detail/note-version-fallback'
import { NoteVersionHeader } from '#components/note-detail/note-version-header'
import { NoteVersionList } from '#components/note-detail/note-version-list'
import { NoteVersionReviewPanel } from '#components/note-detail/note-version-review-panel'
import { PredictionsPanel } from '#components/note-detail/predictions-panel'
import { openFloatingChat } from '#components/strategy-shell/floating-chat-store'
import { $api } from '#lib/api/client'

export const Route = createFileRoute('/notes/$noteId')({
  validateSearch: (search: Record<string, unknown>): { version?: number } => {
    const version = search.version
    if (
      typeof version === 'number' &&
      Number.isInteger(version) &&
      version > 0
    ) {
      return { version }
    }
    if (typeof version === 'string' && /^\d+$/.test(version)) {
      const parsed = Number(version)
      return parsed > 0 ? { version: parsed } : {}
    }
    return {}
  },
  component: NoteDetailPage,
})

function NoteDetailPage() {
  const { noteId } = Route.useParams()
  const { version: requestedVersionNo } = Route.useSearch()
  const navigate = Route.useNavigate()
  const { data: note } = $api.useQuery('get', '/api/notes/{id}', {
    params: { path: { id: noteId } },
  })
  const {
    data: versions,
    isPending: areVersionsPending,
    isError: hasVersionsError,
  } = $api.useQuery('get', '/api/notes/{id}/versions', {
    params: { path: { id: noteId } },
  })

  if (areVersionsPending) {
    return <NoteVersionFallback state="loading" />
  }

  if (hasVersionsError) {
    return <NoteVersionFallback state="error" />
  }

  const orderedVersions = [...versions].sort(
    (left, right) => left.version_no - right.version_no,
  )
  const selectedVersion =
    orderedVersions.find(
      (version) => version.version_no === requestedVersionNo,
    ) ??
    orderedVersions.find((version) => version.is_current) ??
    orderedVersions.at(-1)

  if (selectedVersion == null) {
    return <NoteVersionFallback state="missing" />
  }

  const previousVersion =
    orderedVersions
      .filter((version) => version.version_no < selectedVersion.version_no)
      .at(-1) ?? null

  return (
    <div className="space-y-4 font-sans text-foreground">
      {note?.strategy_id != null && (
        <Link
          to="/strategies/$id/performance"
          params={{ id: note.strategy_id }}
          className="inline-flex items-center gap-1 font-mono text-xs text-muted-foreground hover:text-primary"
        >
          &lt; 戦略成績に戻る
        </Link>
      )}
      <div className="grid grid-cols-1 gap-5 lg:grid-cols-(--grid-cols-note-detail)">
        <article className="space-y-4 border border-border bg-card px-5 py-5">
          <NoteVersionHeader version={selectedVersion} />
          <NoteVersionDiffPanel
            key={selectedVersion.id}
            version={selectedVersion}
            previousVersion={previousVersion}
          />
          <NoteVersionChatAction
            title={selectedVersion.title}
            onAsk={(title) => {
              openFloatingChat(`「${title}」について補足して`)
            }}
          />
        </article>
        <aside className="space-y-4">
          <NoteVersionList
            versions={orderedVersions}
            selectedVersionNo={selectedVersion.version_no}
            onSelectVersion={(versionNo) => {
              void navigate({ search: { version: versionNo } })
            }}
          />
          <NoteVersionReviewPanel
            key={selectedVersion.id}
            version={selectedVersion}
          />
          <PredictionsPanel noteId={noteId} />
          <NoteHypothesesPanel
            noteId={noteId}
            strategyId={note?.strategy_id ?? null}
          />
          <HistoryPanel noteId={noteId} />
        </aside>
      </div>
    </div>
  )
}
