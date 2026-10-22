import { Button } from '@fohte/ui/button'
import { Chip } from '@fohte/ui/chip'

import {
  groupNoteHistoryEvents,
  type HistoryEventData,
} from '#components/note-detail/note-history-events'
import { StatusPill } from '#components/strategy-home/status-pill'
import type { NoteVersion } from '#lib/api/note-version-types'
import type { components } from '#lib/api/schema.gen'
import { formatRelative } from '#lib/note-utils'

type ChangeHistory = components['schemas']['ChangeHistory']

interface NoteHistoryPanelProps {
  versions: NoteVersion[]
  history: ChangeHistory[]
  isHistoryPending: boolean
  isHistoryError: boolean
  selectedVersionNo: number
  onSelectVersion: (versionNo: number) => void
}

function HistoryEvent({ event }: { event: HistoryEventData }) {
  const { entry, label } = event
  return (
    <li className="flex flex-wrap items-baseline gap-x-2 gap-y-1 px-3.5 py-2 font-mono text-2xs text-muted-foreground">
      <time dateTime={entry.created_at}>
        {formatRelative(entry.created_at)}
      </time>
      <span className="text-muted-foreground-strong">
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
  const { orderedVersions, eventsByVersionId, unmatchedEvents } =
    groupNoteHistoryEvents(versions, history)

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
                <Button
                  type="button"
                  variant={
                    version.version_no === selectedVersionNo
                      ? 'default'
                      : 'ghost'
                  }
                  aria-current={
                    version.version_no === selectedVersionNo
                      ? 'true'
                      : undefined
                  }
                  onClick={() => {
                    onSelectVersion(version.version_no)
                  }}
                  className="h-auto w-full flex-col whitespace-normal text-left"
                >
                  <span className="flex w-full flex-col items-center gap-2">
                    <span className="flex items-center justify-between gap-2">
                      <span className="font-mono text-xs font-bold text-foreground">
                        v{String(version.version_no)}
                      </span>
                      {version.is_current && <Chip>現行</Chip>}
                    </span>
                    <span className="line-clamp-2 w-full text-center text-sm leading-snug text-foreground">
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
                  </span>
                </Button>
                {versionEvents.length > 0 && (
                  <ul className="divide-y divide-border border-t border-border bg-bg-tertiary px-2">
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
