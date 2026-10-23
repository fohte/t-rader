import { Chip } from '@fohte/ui/chip'

import { ExecutionOriginLinks } from '#components/execution-origin-links'
import { NoteVersionChatAction } from '#components/note-detail/note-version-chat-action'
import { NoteKindLabel } from '#components/notes/note-kind-label'
import { StatusPill } from '#components/strategy-home/status-pill'
import type { NoteVersion } from '#lib/api/note-version-types'
import type { components } from '#lib/api/schema.gen'
import type { ExecutionOrigin } from '#lib/execution-origin'
import type {
  NoteKindCatalogState,
  NoteKindSourceState,
} from '#lib/note-kind-label'
import { formatRelative } from '#lib/note-utils'

type NoteKind = components['schemas']['NoteKind']

interface NoteVersionHeaderProps {
  version: NoteVersion
  noteKind: string | null
  noteKinds: readonly NoteKind[]
  noteKindCatalogState: NoteKindCatalogState
  noteKindSourceState: NoteKindSourceState
  executionOrigin: ExecutionOrigin | null
  onAsk: (title: string) => void
}

export function NoteVersionHeader({
  version,
  noteKind,
  noteKinds,
  noteKindCatalogState,
  noteKindSourceState,
  executionOrigin,
  onAsk,
}: NoteVersionHeaderProps) {
  return (
    <header className="space-y-3 border-b border-border pb-4">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 space-y-2">
          <NoteKindLabel
            kind={noteKind}
            noteKinds={noteKinds}
            catalogState={noteKindCatalogState}
            sourceState={noteKindSourceState}
            linkToFilter
          />
          <h1 className="break-words text-2xl font-bold leading-tight tracking-tight text-foreground">
            {version.title}
          </h1>
        </div>
        <NoteVersionChatAction title={version.title} onAsk={onAsk} />
      </div>
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2 font-mono text-2xs text-muted-foreground">
        <StatusPill status={version.status} />
        <span>v{String(version.version_no)}</span>
      </div>
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2 font-mono text-2xs text-muted-foreground">
        <span>
          {version.created_by_kind === 'llm' ? 'エージェント' : 'ユーザー'}
        </span>
        {version.created_by_kind === 'llm' && executionOrigin != null && (
          <ExecutionOriginLinks origin={executionOrigin} />
        )}
      </div>
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2 font-mono text-2xs text-muted-foreground">
        {version.is_current && <Chip>現行</Chip>}
        <span>{formatRelative(version.created_at)} 作成</span>
      </div>
      {version.version_no > 1 &&
        version.change_reason != null &&
        version.change_reason !== '' && (
          <p className="whitespace-pre-wrap border-l-2 border-border pl-3 text-sm leading-relaxed text-muted-foreground-strong">
            <span className="font-mono text-2xs text-muted-foreground">
              変更理由:
            </span>
            {version.change_reason}
          </p>
        )}
    </header>
  )
}
