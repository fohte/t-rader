import { Link } from '@tanstack/react-router'

export function PendingNoteVersionsLink() {
  return (
    <Link
      to="/note-versions/pending"
      className="inline-flex items-center gap-1 border border-border px-2.5 py-1 font-mono text-2xs text-muted-foreground-strong hover:border-primary hover:text-primary"
    >
      承認待ち一覧 <span aria-hidden="true">→</span>
    </Link>
  )
}
