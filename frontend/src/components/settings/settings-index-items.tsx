import { Link } from '@tanstack/react-router'

const ITEMS: { to: string; label: string; description: string }[] = [
  {
    to: '/settings/agent-configs',
    label: 'Agent 設定',
    description: '目的ごとの AGENTS.md / skills / agent graph を管理する',
  },
  {
    to: '/settings/rss-feeds',
    label: 'RSS フィード',
    description: 'ニュース集約対象の公開 RSS を追加・編集・無効化する',
  },
  {
    to: '/settings/ingest-status',
    label: '取り込み状況',
    description: '取り込みジョブの実行履歴とデータ状況を確認する',
  },
  {
    to: '/settings/risk-policy',
    label: 'リスク上限',
    description:
      '口座全体の保有銘柄時価合計に対するセクター集中度の上限を設定する',
  },
  {
    to: '/settings/indicators',
    label: '自作指標 (共通)',
    description: 'すべての戦略で使う自作指標を管理する',
  },
]

export function SettingsIndexItems() {
  return (
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
  )
}
