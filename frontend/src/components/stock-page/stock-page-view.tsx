import type { ComponentProps } from 'react'

import { StockPageActivity } from '#components/stock-page/stock-page-activity'
import { StockPageChart } from '#components/stock-page/stock-page-chart'
import { StockPageCompanyInfo } from '#components/stock-page/stock-page-company-info'
import { StockPageHeader } from '#components/stock-page/stock-page-header'
import type { Timeframe } from '#components/timeframe-selector'
import type { components } from '#lib/api/schema.gen'
import type { ChartCurrency } from '#lib/chart-utils'
import type { StockQuote } from '#lib/stock-page-model'

type Stock = components['schemas']['Stock']
type Position = components['schemas']['PositionSummary']
type Annotation = components['schemas']['Annotation']
type Trade = components['schemas']['TradeListItem']
type GroupMembership = components['schemas']['StockGroupMembershipResponse']
type NextEarnings = components['schemas']['StockNextEarningsResponse']
type Valuation = components['schemas']['StockValuationResponse']
type FinancialSummary = components['schemas']['StockFinancialSummaryResponse']
type ActivityProps = ComponentProps<typeof StockPageActivity>

export function StockPageView({
  stock,
  quote,
  quoteLoading,
  quoteError,
  bars,
  annotations,
  trades,
  groups,
  groupsState,
  nextEarnings,
  nextEarningsState,
  valuation,
  valuationState,
  financialSummaries,
  financialSummariesState,
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
  groups: GroupMembership[]
  groupsState: ActivityProps['groupsState']
  nextEarnings: NextEarnings | null
  nextEarningsState: ActivityProps['groupsState']
  valuation: Valuation | null
  valuationState: ActivityProps['groupsState']
  financialSummaries: FinancialSummary[]
  financialSummariesState: ActivityProps['financialSummariesState']
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
  return (
    <main className="space-y-4 font-sans text-foreground sm:space-y-5">
      <StockPageHeader
        stock={stock}
        quote={quote}
        isHolding={position != null}
        groups={groups}
        groupsState={groupsState}
        quoteLoading={quoteLoading}
        quoteError={quoteError}
      />

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-4">
        <div className="lg:col-span-3">
          <StockPageChart
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
        <StockPageCompanyInfo
          position={position}
          holdingsState={
            holdingsLoading ? 'loading' : holdingsError ? 'error' : 'ready'
          }
          earnings={nextEarnings}
          earningsState={nextEarningsState}
          valuation={valuation}
          valuationState={valuationState}
        />
      </div>

      <StockPageActivity
        annotations={annotations}
        trades={trades}
        financialSummaries={financialSummaries}
        financialSummariesState={financialSummariesState}
        groups={groups}
        groupsState={groupsState}
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
