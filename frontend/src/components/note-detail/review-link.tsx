import { Link } from '@tanstack/react-router'

export function ReviewLink() {
  return (
    <Link
      to="/notes"
      search={(previous) => ({ ...previous, view: 'pending' })}
      className="inline-flex items-center gap-1 border border-border px-2.5 py-1 font-mono text-2xs text-muted-foreground-strong hover:border-border-strong hover:text-foreground"
    >
      未読・レビュー待ちへ <span aria-hidden="true">→</span>
    </Link>
  )
}
