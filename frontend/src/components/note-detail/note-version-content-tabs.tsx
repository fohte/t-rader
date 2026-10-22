import { Tabs, TabsList, TabsTab } from '@fohte/ui/tabs'

export type NoteVersionContentTab = 'changes' | 'full'

export function resolveNoteVersionContentTab(
  selectedTab: NoteVersionContentTab | null,
): NoteVersionContentTab {
  return selectedTab ?? 'changes'
}

function isNoteVersionContentTab(
  value: unknown,
): value is NoteVersionContentTab {
  return value === 'changes' || value === 'full'
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
    <Tabs
      value={activeTab}
      onValueChange={(value: unknown) => {
        if (isNoteVersionContentTab(value)) onChange(value)
      }}
    >
      <TabsList aria-label="ノート本文の表示">
        <TabsTab value="changes">変更点</TabsTab>
        <TabsTab value="full">全文</TabsTab>
      </TabsList>
    </Tabs>
  )
}
