import { Link } from '@tanstack/react-router'

interface HistoricalVersionNoticeProps {
  noteId: string
  versionNo: number
}

export function HistoricalVersionNotice({
  noteId,
  versionNo,
}: HistoricalVersionNoticeProps) {
  return (
    <section className="border border-border bg-card px-3.5 py-3 font-mono text-xs">
      <p className="text-muted-foreground">
        過去のバージョン v{String(versionNo)} を表示中です。
      </p>
      <Link
        to="/notes/$noteId"
        params={{ noteId }}
        search={{ version_id: undefined }}
        className="mt-2 inline-block text-primary hover:underline"
      >
        現行バージョンを開く
      </Link>
    </section>
  )
}
