import { StatusPill } from '#components/strategy-home/status-pill'
import type { NoteVersion } from '#lib/api/note-version-types'
import type { components } from '#lib/api/schema.gen'
import { formatRelative } from '#lib/note-utils'

type ChangeHistory = components['schemas']['ChangeHistory']

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

interface NoteHistoryPanelProps {
  versions: NoteVersion[]
  history: ChangeHistory[]
  isHistoryPending: boolean
  selectedVersionNo: number
  onSelectVersion: (versionNo: number) => void
}

function readStatusChangeDiff(
  entry: ChangeHistory,
): Record<string, unknown> | null {
  return isRecord(entry.diff_json) ? entry.diff_json : null
}

function readStatusChangeVersionId(entry: ChangeHistory): string | null {
  const versionId = readStatusChangeDiff(entry)?.version_id
  return typeof versionId === 'string' ? versionId : null
}

function readStatusChangeTarget(entry: ChangeHistory): string | null {
  const target = readStatusChangeDiff(entry)?.to
  return typeof target === 'string' ? target : null
}

function statusChangeLabel(entry: ChangeHistory): string {
  const target = readStatusChangeTarget(entry)
  if (target === 'approved') return '承認'
  if (target === 'rejected') return '却下'
  if (target === 'superseded') return '置き換え'
  return '状態変更'
}

function HistoryEvent({ entry }: { entry: ChangeHistory }) {
  return (
    <li className="flex flex-wrap items-baseline gap-x-2 gap-y-1 px-3.5 py-2 font-mono text-2xs text-muted-foreground">
      <time dateTime={entry.created_at}>
        {formatRelative(entry.created_at)}
      </time>
      <span
        className={
          entry.actor_kind === 'llm'
            ? 'text-primary'
            : 'text-muted-foreground-strong'
        }
      >
        {entry.actor_label}が{statusChangeLabel(entry)}
      </span>
      {entry.summary != null && entry.summary !== '' && (
        <span>{entry.summary}</span>
      )}
    </li>
  )
}

export function NoteHistoryPanel({
  versions,
  history,
  isHistoryPending,
  selectedVersionNo,
  onSelectVersion,
}: NoteHistoryPanelProps) {
  const orderedVersions = [...versions].sort(
    (left, right) => right.version_no - left.version_no,
  )
  const statusChangesByVersionId = new Map<string, ChangeHistory[]>()
  const unmatchedStatusChanges: ChangeHistory[] = []

  for (const entry of history) {
    if (entry.op !== 'status_change') continue
    const versionId = readStatusChangeVersionId(entry)
    if (versionId == null) {
      unmatchedStatusChanges.push(entry)
      continue
    }
    const versionEntries = statusChangesByVersionId.get(versionId) ?? []
    versionEntries.push(entry)
    statusChangesByVersionId.set(versionId, versionEntries)
  }

  for (const entries of statusChangesByVersionId.values()) {
    entries.sort((left, right) =>
      right.created_at.localeCompare(left.created_at),
    )
  }
  unmatchedStatusChanges.sort((left, right) =>
    right.created_at.localeCompare(left.created_at),
  )

  return (
    <section
      className="border border-border bg-card"
      aria-labelledby="note-history-heading"
    >
      <header className="border-b border-border px-3.5 py-2">
        <h2
          id="note-history-heading"
          className="font-mono text-xs font-bold uppercase tracking-wider text-foreground"
        >
          履歴
        </h2>
      </header>
      {orderedVersions.length === 0 ? (
        <p className="px-3.5 py-3 font-mono text-xs text-muted-foreground">
          バージョンがありません。
        </p>
      ) : (
        <ol className="divide-y divide-border">
          {orderedVersions.map((version) => {
            const versionEvents = statusChangesByVersionId.get(version.id) ?? []
            return (
              <li key={version.id}>
                <button
                  type="button"
                  aria-current={
                    version.version_no === selectedVersionNo
                      ? 'true'
                      : undefined
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
                      <span className="font-mono text-2xs text-primary">
                        現行
                      </span>
                    )}
                  </span>
                  <span className="line-clamp-2 text-sm leading-snug text-foreground">
                    {version.title}
                  </span>
                  <span className="flex flex-wrap items-center justify-between gap-2">
                    <StatusPill status={version.status} />
                    <span className="font-mono text-2xs text-muted-foreground">
                      {version.created_by_kind === 'llm'
                        ? 'エージェント'
                        : '自分'}
                      {' · '}
                      {formatRelative(version.created_at)}
                    </span>
                  </span>
                </button>
                {versionEvents.length > 0 && (
                  <ul className="divide-y divide-border border-t border-border bg-surface px-2">
                    {versionEvents.map((entry) => (
                      <HistoryEvent key={entry.id} entry={entry} />
                    ))}
                  </ul>
                )}
              </li>
            )
          })}
          {unmatchedStatusChanges.map((entry) => (
            <HistoryEvent key={entry.id} entry={entry} />
          ))}
        </ol>
      )}
      {isHistoryPending && (
        <div className="border-t border-border px-3.5 py-3 font-mono text-xs text-muted-foreground">
          読み込み中…
        </div>
      )}
    </section>
  )
}
