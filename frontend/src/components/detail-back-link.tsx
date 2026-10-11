import { Link } from '@tanstack/react-router'

type DetailBackLinkProps = {
  label: string
  tab?: 'annotations'
}

export function DetailBackLink({ label, tab }: DetailBackLinkProps) {
  return (
    <Link
      to="/notes"
      search={{ tab }}
      className="inline-flex items-center gap-1 font-mono text-xs text-muted-foreground hover:text-primary"
    >
      &lt; {label}
    </Link>
  )
}
