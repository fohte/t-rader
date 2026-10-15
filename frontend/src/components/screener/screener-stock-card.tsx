import { Link } from '@tanstack/react-router'

import { formatScreenerMetric } from '#components/screener/format'
import { formatYen } from '#components/trades/format'
import type { components } from '#lib/api/schema.gen'

type ScreenerStock = components['schemas']['ScreenerStockResponse']

export function ScreenerStockCard({ stock }: { stock: ScreenerStock }) {
  return (
    <Link
      to="/stocks/$stockId"
      params={{ stockId: stock.id }}
      className="block border border-border bg-card p-3 hover:border-muted-foreground"
    >
      <div className="flex items-baseline justify-between gap-3">
        <span className="min-w-0 truncate font-semibold">{stock.name}</span>
        <span className="shrink-0 text-2xs text-muted-foreground">
          {stock.id}
        </span>
      </div>
      <div className="mt-2 flex items-baseline justify-between gap-2 border-t border-border pt-2">
        <span className="font-semibold tabular-nums">
          {stock.close == null ? '—' : formatYen(stock.close)}
        </span>
        <span className="text-2xs text-muted-foreground">終値</span>
      </div>
      <dl className="mt-2 grid grid-cols-3 gap-2 border-t border-border pt-2 text-2xs">
        {[
          ['PER', formatScreenerMetric(stock.per, '倍')],
          ['PBR', formatScreenerMetric(stock.pbr, '倍')],
          ['配当利回り', formatScreenerMetric(stock.dividend_yield, '%')],
        ].map(([label, value]) => (
          <div key={label}>
            <dt className="text-muted-foreground">{label}</dt>
            <dd className="mt-1 tabular-nums text-foreground">{value}</dd>
          </div>
        ))}
      </dl>
    </Link>
  )
}
