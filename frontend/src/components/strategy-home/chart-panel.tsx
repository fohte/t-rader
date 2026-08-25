import { useEffect, useMemo, useState } from 'react'

import {
  CandlestickChart,
  type ChartAnnotation,
} from '#components/candlestick-chart'
import type { NumberedAnnotation } from '#lib/annotation-utils'
import { $api } from '#lib/api/client'

type Timeframe = '1D' | '1W' | '1M'

interface ChartPanelProps {
  symbols: { code: string; name: string }[]
  /** 現銘柄について採番済みのアノテーション。親で 1 度だけ計算して渡す */
  numberedAnnotations?: NumberedAnnotation[]
  selectedAnnotationId?: string | null
  onSelectAnnotation?: (id: string) => void
  /** 現在表示中の銘柄 (= 描画対象) が変わったときに呼ばれる */
  onSymbolChange?: (code: string) => void
}

function rangeFor(tf: Timeframe): { from: string; to: string } {
  const now = new Date()
  const to = now.toISOString().slice(0, 10)
  const start = new Date(now)
  if (tf === '1D') start.setDate(start.getDate() - 60)
  else if (tf === '1W') start.setDate(start.getDate() - 180)
  else start.setFullYear(start.getFullYear() - 2)
  return { from: start.toISOString().slice(0, 10), to }
}

export function ChartPanel({
  symbols,
  numberedAnnotations,
  selectedAnnotationId,
  onSelectAnnotation,
  onSymbolChange,
}: ChartPanelProps) {
  const [symIdx, setSymIdx] = useState(0)
  const [tf, setTf] = useState<Timeframe>('1D')

  // バー解像度は常に 1d 固定で、レンジ (from-to) だけを切り替える表示。
  const symbolsKey = useMemo(
    () => symbols.map((s) => s.code).join('|'),
    [symbols],
  )
  useEffect(() => {
    setSymIdx(0)
  }, [symbolsKey])

  const symbol = symbols[symIdx]
  const range = useMemo(() => rangeFor(tf), [tf])

  useEffect(() => {
    if (symbol != null) onSymbolChange?.(symbol.code)
  }, [symbol, onSymbolChange])

  const chartAnnotations = useMemo<ChartAnnotation[]>(
    () =>
      (numberedAnnotations ?? []).map((a) => ({
        id: a.id,
        label: a.label,
        timestamp: a.timestamp,
        target_kind: a.target_kind,
        status: a.status,
        text: a.text,
      })),
    [numberedAnnotations],
  )

  const { data: bars, isPending } = $api.useQuery(
    'get',
    '/api/bars',
    {
      params: {
        query: {
          instrument_id: symbol?.code ?? '',
          timeframe: '1d',
          from: range.from,
          to: range.to,
        },
      },
    },
    { enabled: symbol != null },
  )

  if (symbol == null) {
    return (
      <div className="grid h-105 place-items-center border border-border bg-card font-mono text-xs text-muted-foreground">
        シード関心に銘柄が登録されていません
      </div>
    )
  }

  const last = bars && bars.length > 0 ? bars[bars.length - 1] : null
  const prev = bars && bars.length > 1 ? bars[bars.length - 2] : null
  const chg = last != null && prev != null ? last.close - prev.close : 0
  const pct =
    last != null && prev != null && prev.close !== 0
      ? (chg / prev.close) * 100
      : 0
  const isUp = chg >= 0

  return (
    <div className="border border-border bg-card">
      <div className="flex flex-wrap items-center gap-1.5 border-b border-border p-2.5">
        {symbols.map((s, i) => (
          <button
            key={s.code}
            type="button"
            onClick={() => {
              setSymIdx(i)
            }}
            className={`flex items-baseline gap-1.5 border px-2 py-1 font-mono text-xs ${
              i === symIdx
                ? 'border-muted-foreground bg-surface-strong text-foreground'
                : 'border-border text-muted-foreground-strong hover:border-muted-foreground'
            }`}
          >
            <span>{s.name}</span>
            <span className="text-2xs text-muted-foreground">{s.code}</span>
          </button>
        ))}
        <span className="mx-1 h-5 w-px self-center bg-border" />
        {(['1D', '1W', '1M'] as const).map((t) => (
          <button
            key={t}
            type="button"
            onClick={() => {
              setTf(t)
            }}
            className={`border px-2 py-1 font-mono text-xs ${
              t === tf
                ? 'border-muted-foreground bg-surface-strong text-foreground'
                : 'border-border text-muted-foreground-strong hover:border-muted-foreground'
            }`}
          >
            {t}
          </button>
        ))}
        {last != null && (
          <span className="ml-auto flex items-baseline gap-2 font-mono text-xs tabular-nums">
            <span className={isUp ? 'text-up' : 'text-down'}>
              ¥{last.close.toLocaleString()}
            </span>
            <span className={`text-2xs ${isUp ? 'text-up' : 'text-down'}`}>
              {isUp ? '▲' : '▼'} {Math.abs(chg).toFixed(1)} ({isUp ? '+' : ''}
              {pct.toFixed(2)}%)
            </span>
          </span>
        )}
      </div>
      <div className="h-105">
        {isPending ? (
          <div className="grid h-full place-items-center font-mono text-xs text-muted-foreground">
            読み込み中…
          </div>
        ) : bars && bars.length > 0 ? (
          <CandlestickChart
            bars={bars}
            annotations={chartAnnotations}
            selectedAnnotationId={selectedAnnotationId ?? null}
            onSelectAnnotation={onSelectAnnotation}
            className="h-full w-full"
          />
        ) : (
          <div className="grid h-full place-items-center font-mono text-xs text-muted-foreground">
            データなし
          </div>
        )}
      </div>
    </div>
  )
}
