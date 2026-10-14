import type { ComponentProps } from 'react'

import { StockPageActivity } from '#components/stock-page/stock-page-activity'
import { StockPageChart } from '#components/stock-page/stock-page-chart'
import { StockPageHeader } from '#components/stock-page/stock-page-header'
import { StockPageHoldings } from '#components/stock-page/stock-page-holdings'
import type { Timeframe } from '#components/timeframe-selector'
import type { components } from '#lib/api/schema.gen'
import type { ChartCurrency } from '#lib/chart-utils'
import type { StockQuote } from '#lib/stock-page-model'

type Stock = components['schemas']['Stock']
type Position = components['schemas']['PositionSummary']
type Annotation = components['schemas']['Annotation']
type Trade = components['schemas']['TradeListItem']
type ActivityProps = ComponentProps<typeof StockPageActivity>

export function StockPageView({
  stock,
  quote,
  quoteLoading,
  quoteError,
  bars,
  annotations,
  trades,
  position,
  holdingsLoading,
  holdingsError,
  timeframe,
  onTimeframeChange,
  selectedAnnotationId,
  onSelectAnnotation,
  activeTab,
  onTabChange,
  currency,
  chartLoading,
  chartError,
  annotationState,
  tradeState,
}: {
  stock: Stock
  quote: StockQuote | null
  quoteLoading: boolean
  quoteError: boolean
  bars: components['schemas']['Bar'][]
  annotations: Annotation[]
  trades: Trade[]
  position: Position | null
  holdingsLoading: boolean
  holdingsError: boolean
  timeframe: Timeframe
  onTimeframeChange: (timeframe: Timeframe) => void
  selectedAnnotationId: string | null
  onSelectAnnotation: (id: string) => void
  activeTab: ActivityProps['activeTab']
  onTabChange: ActivityProps['onTabChange']
  currency?: ChartCurrency
  chartLoading: boolean
  chartError: boolean
  annotationState: ActivityProps['annotationState']
  tradeState: ActivityProps['tradeState']
}) {
  const hasHoldingsColumn = position != null || holdingsLoading || holdingsError

  return (
    <main className="space-y-4 font-sans text-foreground sm:space-y-5">
      <StockPageHeader
        stock={stock}
        quote={quote}
        isHolding={position != null}
        quoteLoading={quoteLoading}
        quoteError={quoteError}
      />

      <div
        className={
          hasHoldingsColumn
            ? 'grid grid-cols-1 gap-4 lg:grid-cols-4'
            : 'grid grid-cols-1 gap-4'
        }
      >
        <div className={hasHoldingsColumn ? 'lg:col-span-3' : undefined}>
          <StockPageChart
            stockId={stock.id}
            bars={bars}
            annotations={annotations}
            selectedAnnotationId={selectedAnnotationId}
            onSelectAnnotation={onSelectAnnotation}
            timeframe={timeframe}
            onTimeframeChange={onTimeframeChange}
            currency={currency}
            isLoading={chartLoading}
            hasError={chartError}
          />
        </div>
        <StockPageHoldings
          position={position}
          isLoading={holdingsLoading}
          hasError={holdingsError}
        />
      </div>

      <StockPageActivity
        annotations={annotations}
        trades={trades}
        activeTab={activeTab}
        onTabChange={onTabChange}
        selectedAnnotationId={selectedAnnotationId}
        onSelectAnnotation={onSelectAnnotation}
        annotationState={annotationState}
        tradeState={tradeState}
      />
    </main>
  )
}
