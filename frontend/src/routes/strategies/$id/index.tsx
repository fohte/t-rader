import { createFileRoute } from '@tanstack/react-router'

import { EmptyPlaceholder } from '@/components/strategy-shell/empty-placeholder'
import { STRATEGIES_MOCK } from '@/lib/strategy-mock'

export const Route = createFileRoute('/strategies/$id/')({
  component: StrategyHomePage,
})

function StrategyHomePage() {
  const { id } = Route.useParams()
  const strategy = STRATEGIES_MOCK.find((s) => s.id === id)
  return (
    <EmptyPlaceholder
      title={strategy?.name ?? id}
      description={
        strategy?.desc ??
        'この戦略のホームには、関連マクロ・分析カード・チャート・ノート一覧などを集約予定。'
      }
    />
  )
}
