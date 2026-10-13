import { createFileRoute } from '@tanstack/react-router'

import { TradeDetailPage } from '#components/trades/trade-detail-page'

export const Route = createFileRoute('/trades/$tradeId')({
  component: TradeDetailRoute,
})

function TradeDetailRoute() {
  const { tradeId } = Route.useParams()
  return <TradeDetailPage tradeId={tradeId} />
}
