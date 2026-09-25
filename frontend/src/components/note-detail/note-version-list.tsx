import { PendingNoteVersionsLink } from '#components/note-detail/pending-note-versions-link'
import { StatusPill } from '#components/strategy-home/status-pill'
import type { NoteVersion } from '#lib/api/note-version-types'
import { formatRelative } from '#lib/note-utils'

interface NoteVersionListProps {
  versions: NoteVersion[]
  selectedVersionNo: number
  onSelectVersion: (versionNo: number) => void
}

export function NoteVersionList({
  versions,
  selectedVersionNo,
  onSelectVersion,
}: NoteVersionListProps) {
  const orderedVersions = [...versions].sort(
    (left, right) => right.version_no - left.version_no,
  )

  return (
    <section className="border border-border bg-card">
      <header className="flex items-center justify-between gap-2 border-b border-border px-3.5 py-2">
        <h2 className="font-mono text-xs font-bold uppercase tracking-wider text-foreground">
          バージョン
        </h2>
        <PendingNoteVersionsLink />
      </header>
      {orderedVersions.length === 0 ? (
        <p className="px-3.5 py-3 font-mono text-xs text-muted-foreground">
          バージョンがありません。
        </p>
      ) : (
        <div className="divide-y divide-border">
          {orderedVersions.map((version) => (
            <button
              key={version.id}
              type="button"
              aria-current={
                version.version_no === selectedVersionNo ? 'true' : undefined
              }
              onClick={() => {
                onSelectVersion(version.version_no)
              }}
              className={`flex w-full flex-col gap-2 px-3.5 py-3 text-left hover:bg-surface-strong ${version.version_no === selectedVersionNo ? 'bg-surface-strong' : ''}`}
            >
              <span className="flex items-center justify-between gap-2">
                <span className="font-mono text-xs font-bold text-foreground">
                  v{String(version.version_no)}
                </span>
                {version.is_current && (
                  <span className="font-mono text-2xs text-primary">現行</span>
                )}
              </span>
              <span className="line-clamp-2 text-sm leading-snug text-foreground">
                {version.title}
              </span>
              <span className="flex flex-wrap items-center justify-between gap-2">
                <StatusPill status={version.status} />
                <span className="font-mono text-2xs text-muted-foreground">
                  {formatRelative(version.created_at)}
                </span>
              </span>
            </button>
          ))}
        </div>
      )}
    </section>
  )
}
