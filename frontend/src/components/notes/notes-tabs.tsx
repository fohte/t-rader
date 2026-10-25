import { Link } from '@tanstack/react-router'

export type NotesTab = 'notes' | 'annotations'

export function notesTabSearch(tab: NotesTab): { tab?: 'annotations' } {
  return { tab: tab === 'annotations' ? 'annotations' : undefined }
}

const TABS: { key: NotesTab; label: string }[] = [
  { key: 'notes', label: 'ノート' },
  { key: 'annotations', label: 'アノテーション' },
]

export type AnnotationUnreadCountState = 'loading' | 'error' | 'loaded'

export function NotesTabs({
  activeTab,
  annotationUnreadCount,
  annotationUnreadCountState = 'loaded',
}: {
  activeTab: NotesTab
  annotationUnreadCount?: number
  annotationUnreadCountState?: AnnotationUnreadCountState
}) {
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
              ...notesTabSearch(key),
              view:
                prev.view === 'pending' || prev.view === 'all'
                  ? prev.view
                  : undefined,
              kind: prev.kind,
              tag: prev.tag,
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
            {key === 'annotations' &&
              (annotationUnreadCountState === 'loading' ? (
                <span
                  aria-label="未読アノテーション件数を読み込み中"
                  className="border border-border px-1 text-2xs leading-4 tabular-nums text-muted-foreground"
                >
                  …
                </span>
              ) : annotationUnreadCountState === 'error' ? (
                <span
                  aria-label="未読アノテーション件数を取得できません"
                  className="border border-destructive px-1 text-2xs leading-4 tabular-nums text-destructive"
                >
                  —
                </span>
              ) : (annotationUnreadCount ?? 0) > 0 ? (
                <span
                  aria-label={`未読アノテーション ${String(annotationUnreadCount)} 件`}
                  className="border border-primary px-1 text-2xs leading-4 tabular-nums text-primary"
                >
                  {String(annotationUnreadCount)}
                </span>
              ) : null)}
          </Link>
        )
      })}
    </nav>
  )
}
