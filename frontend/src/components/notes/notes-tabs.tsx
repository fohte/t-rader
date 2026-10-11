import { Link } from '@tanstack/react-router'

export type NotesTab = 'notes' | 'annotations'

export function notesTabSearch(tab: NotesTab): { tab?: 'annotations' } {
  return { tab: tab === 'annotations' ? 'annotations' : undefined }
}

const TABS: { key: NotesTab; label: string }[] = [
  { key: 'notes', label: 'ノート' },
  { key: 'annotations', label: 'アノテーション' },
]

export function NotesTabs({ activeTab }: { activeTab: NotesTab }) {
  return (
    <nav
      role="tablist"
      aria-label="ノートとアノテーション"
      className="flex w-full border border-border sm:w-fit"
    >
      {TABS.map(({ key, label }) => {
        const active = activeTab === key

        return (
          <Link
            key={key}
            id={`notes-tab-${key}`}
            to="/notes"
            search={(prev) => ({
              ...prev,
              ...notesTabSearch(key),
            })}
            role="tab"
            aria-selected={active}
            aria-controls="notes-tabpanel"
            data-active={active}
            className="flex min-w-0 flex-1 items-center justify-center gap-1.5 border-r border-border px-3 py-1.5 font-mono text-xs uppercase tracking-wider text-muted-foreground last:border-r-0 hover:text-foreground data-[active=true]:bg-surface-strong data-[active=true]:text-foreground sm:flex-none"
          >
            <span
              aria-hidden="true"
              className={active ? 'text-primary' : 'invisible'}
            >
              &gt;
            </span>
            {label}
          </Link>
        )
      })}
    </nav>
  )
}
