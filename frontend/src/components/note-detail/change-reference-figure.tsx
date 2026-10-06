import { ChangeReferenceFigureView } from '#components/note-detail/change-reference-figure-view'
import { $api } from '#lib/api/client'
import type { ParsedNoteChangeReference } from '#lib/note-change-reference'

export function ChangeReferenceFigure(reference: ParsedNoteChangeReference) {
  const result = $api.useQuery('get', '/api/bars', {
    params: {
      query: {
        instrument_id: reference.instrumentId,
        timeframe: '1d',
        from: reference.start,
        to: reference.end,
      },
    },
  })

  const bars = result.data ?? []
  const state = result.isPending
    ? { status: 'loading' as const }
    : result.isError
      ? { status: 'error' as const }
      : bars.length === 0
        ? { status: 'empty' as const }
        : { status: 'ready' as const, bars }

  return (
    <ChangeReferenceFigureView
      instrumentId={reference.instrumentId}
      start={reference.start}
      end={reference.end}
      state={state}
    />
  )
}
