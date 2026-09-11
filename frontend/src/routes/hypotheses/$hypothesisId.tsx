import { createFileRoute } from '@tanstack/react-router'

import { HypothesisDetailPage } from '#components/hypothesis-detail/hypothesis-detail-page'

export const Route = createFileRoute('/hypotheses/$hypothesisId')({
  component: RouteComponent,
})

function RouteComponent() {
  const { hypothesisId } = Route.useParams()
  return <HypothesisDetailPage hypothesisId={hypothesisId} />
}
