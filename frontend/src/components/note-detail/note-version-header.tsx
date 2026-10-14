import { NoteVersionChatAction } from '#components/note-detail/note-version-chat-action'
import { StatusPill } from '#components/strategy-home/status-pill'
import type { NoteVersion } from '#lib/api/note-version-types'
import { formatRelative } from '#lib/note-utils'

interface NoteVersionHeaderProps {
  version: NoteVersion
  noteKind: string | null
  onAsk: (title: string) => void
}

export function NoteVersionHeader({
  version,
  noteKind,
  onAsk,
}: NoteVersionHeaderProps) {
  return (
    <header className="space-y-3 border-b border-border pb-4">
      <div className="flex flex-wrap items-center justify-between gap-2">
        <div className="flex flex-wrap items-center gap-x-3 gap-y-2 font-mono text-2xs text-muted-foreground">
          <span>
            {version.created_by_kind === 'llm' ? 'エージェント' : 'ユーザー'}
          </span>
          <span>{noteKind ?? '種別なし'}</span>
          <span>v{String(version.version_no)}</span>
          <StatusPill status={version.status} />
        </div>
        <NoteVersionChatAction title={version.title} onAsk={onAsk} />
      </div>
      <h1 className="text-2xl font-bold leading-tight tracking-tight text-foreground">
        {version.title}
      </h1>
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2 font-mono text-2xs text-muted-foreground">
        {version.is_current && (
          <span className="border border-primary px-1.5 py-px text-primary">
            現行
          </span>
        )}
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
