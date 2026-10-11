import { Link } from '@tanstack/react-router'

import {
  NAV_PILL_ACTIVE,
  NAV_PILL_INACTIVE,
} from '#components/nav-pill-classes'

const tabs = [
  { id: 'holdings', label: '保有', to: '/portfolio' },
  { id: 'trades', label: '取引', to: '/trades' },
] as const
type AssetTab = (typeof tabs)[number]['id']

export function AssetTabs({ activeTab }: { activeTab: AssetTab }) {
  return (
    <nav
      aria-label="資産の表示切替"
      className="flex flex-wrap items-center gap-2"
    >
      {tabs.map(({ id, label, to }) => (
        <Link
          key={id}
          to={to}
          className={activeTab === id ? NAV_PILL_ACTIVE : NAV_PILL_INACTIVE}
          aria-current={activeTab === id ? 'page' : undefined}
        >
          {label}
        </Link>
      ))}
    </nav>
  )
}
