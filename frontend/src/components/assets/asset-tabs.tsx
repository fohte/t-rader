import { Link } from '@tanstack/react-router'

export type AssetTab = 'holdings' | 'trades'

const tabs: { id: AssetTab; label: string }[] = [
  { id: 'holdings', label: '保有' },
  { id: 'trades', label: '取引' },
]

const TAB_BASE =
  'flex flex-shrink-0 items-center whitespace-nowrap border px-2.5 py-1 font-mono text-xs'
const TAB_INACTIVE = `${TAB_BASE} border-border text-muted-foreground-strong hover:border-muted-foreground hover:text-foreground`
const TAB_ACTIVE = `${TAB_BASE} border-muted-foreground bg-surface-strong text-foreground`

export function AssetTabs({ activeTab }: { activeTab: AssetTab }) {
  return (
    <nav
      aria-label="資産の表示切替"
      className="flex flex-wrap items-center gap-2"
    >
      {tabs.map(({ id, label }) => (
        <Link
          key={id}
          to={id === 'holdings' ? '/portfolio' : '/trades'}
          className={activeTab === id ? TAB_ACTIVE : TAB_INACTIVE}
          aria-current={activeTab === id ? 'page' : undefined}
        >
          {label}
        </Link>
      ))}
    </nav>
  )
}
