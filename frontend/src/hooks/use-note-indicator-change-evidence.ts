import { useQuery } from '@tanstack/react-query'

import { fetchClient } from '#lib/api/client'
import {
  type NoteIndicatorChangeEvidenceState,
  resolveNoteIndicatorChangeEvidence,
} from '#lib/note-indicator-change-evidence'

async function fetchIndicatorObservation(
  indicatorId: string,
  date: string,
  signal: AbortSignal,
) {
  return fetchClient.GET('/api/refs/indicators/{id}/observations', {
    params: {
      path: { id: indicatorId },
      query: { from: date, to: date },
    },
    signal,
  })
}

export function useNoteIndicatorChangeEvidence(
  indicatorId: string,
  start: string,
  end: string,
  enabled: boolean,
): NoteIndicatorChangeEvidenceState {
  const query = useQuery({
    queryKey: ['note-indicator-change-evidence', indicatorId, start, end],
    queryFn: async ({ signal }) => {
      const [startResult, endResult] = await Promise.all([
        fetchIndicatorObservation(indicatorId, start, signal),
        fetchIndicatorObservation(indicatorId, end, signal),
      ])

      if (
        !startResult.response.ok ||
        startResult.data == null ||
        !endResult.response.ok ||
        endResult.data == null
      ) {
        return { status: 'error' as const }
      }

      return resolveNoteIndicatorChangeEvidence(
        startResult.data,
        endResult.data,
        start,
        end,
      )
    },
    enabled,
    retry: false,
  })

  return query.isPending
    ? { status: 'loading' }
    : query.isError
      ? { status: 'error' }
      : query.data
}
