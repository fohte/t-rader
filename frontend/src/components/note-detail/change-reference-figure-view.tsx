import { CandlestickChart } from '#components/candlestick-chart'
import type { components } from '#lib/api/schema.gen'

type Bar = components['schemas']['Bar']

export type ChangeReferenceFigureState =
  | { status: 'loading' }
  | { status: 'error' }
  | { status: 'empty' }
  | { status: 'ready'; bars: Bar[] }

interface ChangeReferenceFigureViewProps {
  instrumentId: string
  start: string
  end: string
  state: ChangeReferenceFigureState
}

export function ChangeReferenceFigureView({
  instrumentId,
  start,
  end,
  state,
}: ChangeReferenceFigureViewProps) {
  return (
    <figure className="my-2 border border-border bg-background">
      <figcaption className="border-b border-border px-3 py-1.5 font-mono text-2xs text-muted-foreground">
        {instrumentId} · {start} – {end}
      </figcaption>
      {state.status === 'ready' ? (
        <CandlestickChart bars={state.bars} className="h-48 w-full" />
      ) : state.status === 'loading' ? (
        <div
          aria-label="区間のチャートを読み込み中"
          role="status"
          className="h-48 animate-pulse bg-surface-strong"
        />
      ) : (
        <p role="status" className="px-3 py-4 text-xs text-muted-foreground">
          {state.status === 'empty'
            ? 'この期間のチャートデータがありません'
            : '区間のチャートを表示できません'}
        </p>
      )}
    </figure>
  )
}
