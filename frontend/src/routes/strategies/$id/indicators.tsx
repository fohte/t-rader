import { createFileRoute } from '@tanstack/react-router'

import { IndicatorsPage } from '#components/indicators/indicators-page'

export const Route = createFileRoute('/strategies/$id/indicators')({
  component: StrategyIndicatorsRoute,
})

function StrategyIndicatorsRoute() {
  const { id } = Route.useParams()
  return <IndicatorsPage scope="strategy" strategyId={id} />
}
