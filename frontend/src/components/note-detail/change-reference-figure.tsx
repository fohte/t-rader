import { ChangeReferenceFigureView } from '#components/note-detail/change-reference-figure-view'
import { $api } from '#lib/api/client'

interface ChangeReferenceFigureProps {
  token: string
}

interface ChangeReference {
  instrumentId: string
  start: string
  end: string
}

const CHANGE_REFERENCE_RE =
  /^\[\[change:([^@\s]+)@(\d{4}-\d{2}-\d{2})\.\.(\d{4}-\d{2}-\d{2}):(open|high|low|close|volume)\]\]$/

function isDate(value: string): boolean {
  const timestamp = Date.parse(`${value}T00:00:00.000Z`)
  return (
    Number.isFinite(timestamp) &&
    new Date(timestamp).toISOString().slice(0, 10) === value
  )
}

function parseChangeReference(token: string): ChangeReference | undefined {
  const match = CHANGE_REFERENCE_RE.exec(token)
  const instrumentId = match?.[1]
  const start = match?.[2]
  const end = match?.[3]
  if (
    instrumentId == null ||
    instrumentId.length === 0 ||
    start == null ||
    end == null ||
    !isDate(start) ||
    !isDate(end) ||
    start >= end
  ) {
    return undefined
  }
  return { instrumentId, start, end }
}

export function ChangeReferenceFigure({ token }: ChangeReferenceFigureProps) {
  const reference = parseChangeReference(token)
  const result = $api.useQuery(
    'get',
    '/api/bars',
    {
      params: {
        query: {
          instrument_id: reference?.instrumentId ?? '',
          timeframe: '1d',
          from: reference?.start ?? '',
          to: reference?.end ?? '',
        },
      },
    },
    { enabled: reference != null },
  )

  if (reference == null) return null

  const state = result.isPending
    ? { status: 'loading' as const }
    : result.isError
      ? { status: 'error' as const }
      : result.data.length === 0
        ? { status: 'empty' as const }
        : { status: 'ready' as const, bars: result.data }

  return (
    <ChangeReferenceFigureView
      instrumentId={reference.instrumentId}
      start={reference.start}
      end={reference.end}
      state={state}
    />
  )
}
