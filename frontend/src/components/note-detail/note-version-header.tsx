import { StatusPill } from '#components/strategy-home/status-pill'
import type { NoteVersion } from '#lib/api/note-version-types'
import { formatRelative } from '#lib/note-utils'

interface NoteVersionHeaderProps {
  version: NoteVersion
}

export function NoteVersionHeader({ version }: NoteVersionHeaderProps) {
  return (
    <header className="mb-5 border-b border-border pb-4">
      <h1 className="mb-3 text-2xl font-bold leading-tight tracking-tight text-foreground">
        {version.title}
      </h1>
      <div className="flex flex-wrap items-center gap-x-3 gap-y-2 font-mono text-2xs text-muted-foreground">
        <span>バージョン {String(version.version_no)}</span>
        <StatusPill status={version.status} />
        {version.is_current && (
          <span className="border border-primary px-1.5 py-px text-primary">
            現行
          </span>
        )}
        <span>
          {version.created_by_kind === 'llm'
            ? 'analyst が作成'
            : 'ユーザー が作成'}
        </span>
        <span>{formatRelative(version.created_at)} 作成</span>
      </div>
      {version.change_reason != null && version.change_reason !== '' && (
        <p className="mt-3 whitespace-pre-wrap border-l-2 border-border pl-3 text-sm leading-relaxed text-muted-foreground-strong">
          {version.change_reason}
        </p>
      )}
    </header>
  )
}
