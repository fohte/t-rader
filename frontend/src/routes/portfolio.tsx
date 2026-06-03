import { createFileRoute } from '@tanstack/react-router'

import { EmptyPlaceholder } from '@/components/strategy-shell/empty-placeholder'

export const Route = createFileRoute('/portfolio')({
  component: PortfolioPage,
})

function PortfolioPage() {
  return (
    <EmptyPlaceholder
      title="ポートフォリオ"
      description="戦略横断のポートフォリオビュー。現金比率・全保有銘柄・全体損益・リスク露出を表示予定。"
    />
  )
}
