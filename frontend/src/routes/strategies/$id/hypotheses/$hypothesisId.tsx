import { createFileRoute } from '@tanstack/react-router'

import { HypothesisDetailPage } from '#components/hypothesis-detail/hypothesis-detail-page'

export const Route = createFileRoute(
  '/strategies/$id/hypotheses/$hypothesisId',
)({
  component: RouteComponent,
})

function RouteComponent() {
  const { id, hypothesisId } = Route.useParams()
  return <HypothesisDetailPage strategyId={id} hypothesisId={hypothesisId} />
}
