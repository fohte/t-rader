import { createFileRoute } from '@tanstack/react-router'

import { IndicatorsPage } from '@/components/indicators/indicators-page'

export const Route = createFileRoute('/indicators')({
  component: GlobalIndicatorsRoute,
})

function GlobalIndicatorsRoute() {
  return <IndicatorsPage scope="global" />
}
