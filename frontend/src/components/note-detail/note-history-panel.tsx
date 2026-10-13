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
  isHistoryError: boolean
  selectedVersionNo: number
  onSelectVersion: (versionNo: number) => void
}

interface HistoryEventData {
  entry: ChangeHistory
  label: string
}

function readDiff(entry: ChangeHistory): Record<string, unknown> | null {
  return isRecord(entry.diff_json) ? entry.diff_json : null
}

function readString(entry: ChangeHistory, key: string): string | null {
  const value = readDiff(entry)?.[key]
  return typeof value === 'string' ? value : null
}

function readStatusChangeVersionId(entry: ChangeHistory): string | null {
  return readString(entry, 'version_id')
}

function readSupersededVersionIds(entry: ChangeHistory): string[] {
  const ids = readDiff(entry)?.superseded_version_ids
  return Array.isArray(ids)
    ? ids.filter((id): id is string => typeof id === 'string')
    : []
}

function statusChangeLabel(entry: ChangeHistory): string {
  const target = readString(entry, 'to')
  if (target === 'approved') return '承認'
  if (target === 'rejected') return '却下'
  if (target === 'superseded') return '置き換え'
  return '状態変更'
}

function operationLabel(entry: ChangeHistory): string {
  if (entry.op === 'create') return '作成'
  if (entry.op === 'update') return '更新'
  if (entry.op === 'delete') return '削除'
  return '変更'
}

function HistoryEvent({ event }: { event: HistoryEventData }) {
  const { entry, label } = event
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
        {entry.actor_label}が{label}
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
  isHistoryError,
  selectedVersionNo,
  onSelectVersion,
}: NoteHistoryPanelProps) {
  const orderedVersions = [...versions].sort(
    (left, right) => right.version_no - left.version_no,
  )
  const versionsById = new Map(
    orderedVersions.map((version) => [version.id, version]),
  )
  const latestVersionNo = orderedVersions[0]?.version_no
  const eventsByVersionId = new Map<string, HistoryEventData[]>()
  const unmatchedEvents: HistoryEventData[] = []

  const addEvent = (versionId: string | null, event: HistoryEventData) => {
    if (versionId == null || !versionsById.has(versionId)) {
      unmatchedEvents.push(event)
      return
    }
    const versionEvents = eventsByVersionId.get(versionId) ?? []
    versionEvents.push(event)
    eventsByVersionId.set(versionId, versionEvents)
  }

  for (const entry of history) {
    if (entry.op === 'status_change') {
      const event = { entry, label: statusChangeLabel(entry) }
      const versionId = readStatusChangeVersionId(entry)
      addEvent(versionId, event)
      for (const supersededVersionId of readSupersededVersionIds(entry)) {
        if (supersededVersionId !== versionId) {
          addEvent(supersededVersionId, { entry, label: '置き換え' })
        }
      }
      continue
    }

    const toVersionId = readString(entry, 'to_version_id')
    const targetVersion =
      toVersionId == null ? undefined : versionsById.get(toVersionId)
    const isCurrentVersionChange =
      entry.op === 'update' &&
      targetVersion != null &&
      latestVersionNo != null &&
      targetVersion.version_no < latestVersionNo
    addEvent(toVersionId, {
      entry,
      label: isCurrentVersionChange ? '現行に変更' : operationLabel(entry),
    })
  }

  for (const events of eventsByVersionId.values()) {
    events.sort((left, right) =>
      right.entry.created_at.localeCompare(left.entry.created_at),
    )
  }
  unmatchedEvents.sort((left, right) =>
    right.entry.created_at.localeCompare(left.entry.created_at),
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
      {orderedVersions.length === 0 && (
        <p className="px-3.5 py-3 font-mono text-xs text-muted-foreground">
          バージョンがありません。
        </p>
      )}
      {(orderedVersions.length > 0 || unmatchedEvents.length > 0) && (
        <ol className="divide-y divide-border">
          {orderedVersions.map((version) => {
            const versionEvents = eventsByVersionId.get(version.id) ?? []
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
                    {versionEvents.map((event, index) => (
                      <HistoryEvent
                        key={`${event.entry.id}:${event.label}:${String(index)}`}
                        event={event}
                      />
                    ))}
                  </ul>
                )}
              </li>
            )
          })}
          {unmatchedEvents.map((event) => (
            <HistoryEvent key={event.entry.id} event={event} />
          ))}
        </ol>
      )}
      {isHistoryError ? (
        <div className="border-t border-border px-3.5 py-3 font-mono text-xs text-muted-foreground">
          履歴を読み込めませんでした。
        </div>
      ) : isHistoryPending ? (
        <div className="border-t border-border px-3.5 py-3 font-mono text-xs text-muted-foreground">
          読み込み中…
        </div>
      ) : null}
    </section>
  )
}
