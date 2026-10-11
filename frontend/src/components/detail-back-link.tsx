import { Link } from '@tanstack/react-router'

type DetailBackLinkProps = {
  to: '/notes' | '/annotations'
  label: string
}

export function DetailBackLink({ to, label }: DetailBackLinkProps) {
  return (
    <Link
      to={to}
      className="inline-flex items-center gap-1 font-mono text-xs text-muted-foreground hover:text-primary"
    >
      &lt; {label}
    </Link>
  )
}
