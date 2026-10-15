import type { ComponentProps } from 'react'

import { StockPageActivity } from '#components/stock-page/stock-page-activity'
import { StockPageChart } from '#components/stock-page/stock-page-chart'
import { StockPageCompanyInfo } from '#components/stock-page/stock-page-company-info'
import { StockPageHeader } from '#components/stock-page/stock-page-header'
import type { Timeframe } from '#components/timeframe-selector'
import type { components } from '#lib/api/schema.gen'
import type { ChartCurrency } from '#lib/chart-utils'
import type { StockPageListState, StockQuote } from '#lib/stock-page-model'

type Stock = components['schemas']['Stock']
type Bar = components['schemas']['Bar']
type Position = components['schemas']['PositionSummary']
type Annotation = components['schemas']['Annotation']
type Note = components['schemas']['Note']
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
  displayRangeStart,
  annotations,
  notes,
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
  holdingsState,
  timeframe,
  onTimeframeChange,
  selectedAnnotationId,
  onSelectAnnotation,
  activeTab,
  onTabChange,
  currency,
  chartLoading,
  chartError,
  selectedDate,
  selectedDateLoading,
  selectedDateError,
  selectedDateBar,
  annotationState,
  noteState,
  tradeState,
}: {
  stock: Stock
  quote: StockQuote | null
  quoteLoading: boolean
  quoteError: boolean
  bars: components['schemas']['Bar'][]
  displayRangeStart?: string
  annotations: Annotation[]
  notes: Note[]
  trades: Trade[]
  groups: GroupMembership[]
  groupsState: StockPageListState
  nextEarnings: NextEarnings | null
  nextEarningsState: StockPageListState
  valuation: Valuation | null
  valuationState: StockPageListState
  financialSummaries: FinancialSummary[]
  financialSummariesState: StockPageListState
  position: Position | null
  holdingsState: StockPageListState
  timeframe: Timeframe
  onTimeframeChange: (timeframe: Timeframe) => void
  selectedAnnotationId: string | null
  onSelectAnnotation: (id: string) => void
  activeTab: ActivityProps['activeTab']
  onTabChange: ActivityProps['onTabChange']
  currency?: ChartCurrency
  chartLoading: boolean
  chartError: boolean
  selectedDate?: string
  selectedDateLoading?: boolean
  selectedDateError?: boolean
  selectedDateBar?: Bar | null
  annotationState: ActivityProps['annotationState']
  noteState: ActivityProps['noteState']
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
            stockId={stock.id}
            bars={bars}
            displayRangeStart={displayRangeStart}
            trades={trades}
            annotations={annotations}
            selectedAnnotationId={selectedAnnotationId}
            onSelectAnnotation={onSelectAnnotation}
            timeframe={timeframe}
            onTimeframeChange={onTimeframeChange}
            currency={currency}
            isLoading={chartLoading}
            hasError={chartError}
            selectedDate={selectedDate}
            selectedDateLoading={selectedDateLoading}
            selectedDateError={selectedDateError}
            selectedDateBar={selectedDateBar}
          />
        </div>
        <StockPageCompanyInfo
          position={position}
          holdingsState={holdingsState}
          earnings={nextEarnings}
          earningsState={nextEarningsState}
          valuation={valuation}
          valuationState={valuationState}
        />
      </div>

      <StockPageActivity
        annotations={annotations}
        notes={notes}
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
        noteState={noteState}
        tradeState={tradeState}
      />
    </main>
  )
}
