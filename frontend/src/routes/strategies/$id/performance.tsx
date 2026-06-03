import { createFileRoute } from '@tanstack/react-router'

import { EmptyPlaceholder } from '@/components/strategy-shell/empty-placeholder'

export const Route = createFileRoute('/strategies/$id/performance')({
  component: StrategyPerformancePage,
})

function StrategyPerformancePage() {
  return (
    <EmptyPlaceholder
      title="戦略成績"
      description="戦略単位の損益・勝率・最大 DD・保有銘柄・トレード一覧を表示予定。"
    />
  )
}
