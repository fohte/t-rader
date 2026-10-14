export type NoteVersionContentTab = 'changes' | 'full'

export function resolveNoteVersionContentTab(
  isReviewing: boolean,
  selectedTab: NoteVersionContentTab | null,
): NoteVersionContentTab {
  return selectedTab ?? (isReviewing ? 'changes' : 'full')
}

interface NoteVersionContentTabsProps {
  activeTab: NoteVersionContentTab
  onChange: (tab: NoteVersionContentTab) => void
}

export function NoteVersionContentTabs({
  activeTab,
  onChange,
}: NoteVersionContentTabsProps) {
  return (
    <div
      role="tablist"
      aria-label="ノート本文の表示"
      className="flex border-b border-border font-mono text-xs"
    >
      <button
        type="button"
        role="tab"
        aria-selected={activeTab === 'changes'}
        onClick={() => {
          onChange('changes')
        }}
        className={`border-b-2 px-3 py-2 ${activeTab === 'changes' ? 'border-primary text-foreground' : 'border-transparent text-muted-foreground hover:text-foreground'}`}
      >
        変更点
      </button>
      <button
        type="button"
        role="tab"
        aria-selected={activeTab === 'full'}
        onClick={() => {
          onChange('full')
        }}
        className={`border-b-2 px-3 py-2 ${activeTab === 'full' ? 'border-primary text-foreground' : 'border-transparent text-muted-foreground hover:text-foreground'}`}
      >
        全文
      </button>
    </div>
  )
}
