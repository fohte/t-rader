import { Link } from '@tanstack/react-router'

import { type NotesTab, notesTabSearch } from '#components/notes/notes-tabs'

type DetailBackLinkProps = {
  label: string
  tab?: NotesTab
}

export function DetailBackLink({ label, tab }: DetailBackLinkProps) {
  return (
    <Link
      to="/notes"
      search={notesTabSearch(tab ?? 'notes')}
      className="inline-flex items-center gap-1 font-mono text-xs text-muted-foreground hover:text-foreground"
    >
      &lt; {label}
    </Link>
  )
}
