import { useEffect, useMemo, useState } from 'react'

import { CandlestickChart } from '@/components/candlestick-chart'
import { $api } from '@/lib/api/client'

type Timeframe = '1D' | '1W' | '1M'

interface ChartPanelProps {
  symbols: { code: string; name: string }[]
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

export function ChartPanel({ symbols }: ChartPanelProps) {
  const [symIdx, setSymIdx] = useState(0)
  const [tf, setTf] = useState<Timeframe>('1D')

  useEffect(() => {
    setSymIdx(0)
  }, [symbols.map((s) => s.code).join(',')])

  const symbol = symbols[symIdx]
  const range = useMemo(() => rangeFor(tf), [tf])

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
      <div className="grid h-[420px] place-items-center border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)] font-mono text-[12px] text-[color:var(--color-text-tertiary)]">
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
    <div className="border border-[color:var(--color-border-strategy)] bg-[color:var(--panel)]">
      <div className="flex flex-wrap items-center gap-1.5 border-b border-[color:var(--color-hairline)] p-2.5">
        {symbols.map((s, i) => (
          <button
            key={s.code}
            type="button"
            onClick={() => {
              setSymIdx(i)
            }}
            className={`flex items-baseline gap-1.5 border px-2 py-1 font-mono text-[12px] ${
              i === symIdx
                ? 'border-[color:var(--color-text-tertiary)] bg-[color:var(--panel-inset)] text-[color:var(--color-text-primary)]'
                : 'border-[color:var(--color-border-strategy)] text-[color:var(--color-text-secondary)] hover:border-[color:var(--color-text-tertiary)]'
            }`}
          >
            <span>{s.name}</span>
            <span className="text-[10px] text-[color:var(--color-text-tertiary)]">
              {s.code}
            </span>
          </button>
        ))}
        <span className="mx-1 h-5 w-px self-center bg-[color:var(--color-hairline)]" />
        {(['1D', '1W', '1M'] as const).map((t) => (
          <button
            key={t}
            type="button"
            onClick={() => {
              setTf(t)
            }}
            className={`border px-2 py-1 font-mono text-[12px] ${
              t === tf
                ? 'border-[color:var(--color-text-tertiary)] bg-[color:var(--panel-inset)] text-[color:var(--color-text-primary)]'
                : 'border-[color:var(--color-border-strategy)] text-[color:var(--color-text-secondary)] hover:border-[color:var(--color-text-tertiary)]'
            }`}
          >
            {t}
          </button>
        ))}
        {last != null && (
          <span className="ml-auto flex items-baseline gap-2 font-mono text-[12px] tabular-nums">
            <span
              className={
                isUp
                  ? 'text-[color:var(--color-up)]'
                  : 'text-[color:var(--color-down)]'
              }
            >
              ¥{last.close.toLocaleString()}
            </span>
            <span
              className={`text-[11px] ${isUp ? 'text-[color:var(--color-up)]' : 'text-[color:var(--color-down)]'}`}
            >
              {isUp ? '▲' : '▼'} {Math.abs(chg).toFixed(1)} ({isUp ? '+' : ''}
              {pct.toFixed(2)}%)
            </span>
          </span>
        )}
      </div>
      <div className="h-[420px]">
        {isPending ? (
          <div className="grid h-full place-items-center font-mono text-[12px] text-[color:var(--color-text-tertiary)]">
            読み込み中…
          </div>
        ) : bars && bars.length > 0 ? (
          <CandlestickChart bars={bars} className="h-full w-full" />
        ) : (
          <div className="grid h-full place-items-center font-mono text-[12px] text-[color:var(--color-text-tertiary)]">
            データなし
          </div>
        )}
      </div>
    </div>
  )
}
