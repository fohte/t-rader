import { createFileRoute, Link } from '@tanstack/react-router'

export const Route = createFileRoute('/settings/')({
  component: SettingsIndexPage,
})

const ITEMS: { to: string; label: string; description: string }[] = [
  {
    to: '/settings/rss-feeds',
    label: 'RSS フィード',
    description: 'ニュース集約対象の公開 RSS を追加・編集・無効化する',
  },
]

function SettingsIndexPage() {
  return (
    <div className="space-y-5">
      <header>
        <h1 className="mb-1 text-2xl font-bold leading-tight tracking-tight">
          設定
        </h1>
        <p className="text-sm text-muted-foreground-strong">
          グローバル設定の一覧。
        </p>
      </header>
      <ul className="space-y-2">
        {ITEMS.map((item) => (
          <li key={item.to}>
            <Link
              to={item.to}
              className="flex items-center justify-between gap-3 border border-border bg-card px-4 py-3 hover:border-muted-foreground"
            >
              <div>
                <div className="font-mono text-sm text-foreground">
                  {item.label}
                </div>
                <div className="text-xs text-muted-foreground-strong">
                  {item.description}
                </div>
              </div>
              <span className="font-mono text-xs text-muted-foreground">
                &gt;
              </span>
            </Link>
          </li>
        ))}
      </ul>
    </div>
  )
}
