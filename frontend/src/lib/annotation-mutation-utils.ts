import {
  type HorizontalLineAnnotationDraft,
  orderTrendLinePoints,
  type TrendLineAnnotationDraft,
} from '#lib/annotation-chart-utils'
import type { components } from '#lib/api/schema.gen'

type CreateAnnotationVariables = {
  body: components['schemas']['CreateAnnotationRequest']
}

type DeleteAnnotationVariables = {
  params: { path: { id: string } }
}

export function saveHorizontalLineAnnotation(
  mutate: (variables: CreateAnnotationVariables) => void,
  targetSymbol: string,
  draft: HorizontalLineAnnotationDraft,
) {
  mutate({
    body: {
      target_kind: draft.target_kind,
      target_symbol: targetSymbol,
      text: draft.text,
      timestamp: draft.timestamp,
      price: draft.price,
      created_by_kind: 'human',
      status: 'approved',
    },
  })
}

export function saveTrendLineAnnotation(
  mutate: (variables: CreateAnnotationVariables) => void,
  targetSymbol: string,
  draft: TrendLineAnnotationDraft,
) {
  const [start, end] = orderTrendLinePoints(draft.start, draft.end)

  mutate({
    body: {
      target_kind: draft.target_kind,
      target_symbol: targetSymbol,
      text: draft.text,
      timestamp_start: start.timestamp,
      price_start: start.price,
      timestamp: end.timestamp,
      price: end.price,
      created_by_kind: 'human',
      status: 'approved',
    },
  })
}

export function deleteChartAnnotation(
  mutate: (variables: DeleteAnnotationVariables) => void,
  id: string,
) {
  mutate({ params: { path: { id } } })
}
