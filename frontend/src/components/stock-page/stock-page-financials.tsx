import { StockPageGroups } from '#components/stock-page/stock-page-groups'
import type { components } from '#lib/api/schema.gen'
import {
  formatDecimal,
  formatYenInHundredMillions,
} from '#lib/stock-page-format'
import {
  sortFinancialSummariesNewestFirst,
  type StockPageListState,
} from '#lib/stock-page-model'

type FinancialSummary = components['schemas']['StockFinancialSummaryResponse']
type GroupMembership = components['schemas']['StockGroupMembershipResponse']
const disclosureDateFormatter = new Intl.DateTimeFormat('ja-JP', {
  year: 'numeric',
  month: '2-digit',
  day: '2-digit',
  timeZone: 'Asia/Tokyo',
})

const PERIOD_LABELS: Record<string, string> = {
  FY: '通期',
  '1Q': '1Q',
  '2Q': '2Q',
  '3Q': '3Q',
  '4Q': '4Q',
}

function formatDisclosureDate(date: string): string {
  return disclosureDateFormatter.format(new Date(`${date}T00:00:00+09:00`))
}

function formatPeriod(summary: FinancialSummary): string {
  const periodEnd =
    summary.current_fiscal_year_end ?? summary.current_period_end
  const yearMonth =
    periodEnd == null
      ? null
      : `${periodEnd.slice(2, 4)}/${String(Number(periodEnd.slice(5, 7)))}`
  const periodType =
    summary.current_period_type == null
      ? null
      : (PERIOD_LABELS[summary.current_period_type] ??
        summary.current_period_type)

  return [yearMonth, periodType].filter((part) => part != null).join(' ') || '—'
}

function FinancialSummaryTable({
  summaries,
}: {
  summaries: FinancialSummary[]
}) {
  const orderedSummaries = sortFinancialSummariesNewestFirst(summaries)

  return (
    <div className="overflow-x-auto">
      <table className="min-w-max w-full font-mono text-xs">
        <caption className="sr-only">
          決算短信の要約。開示日の新しい順。
        </caption>
        <thead>
          <tr className="border-b border-border text-2xs text-muted-foreground">
            <th className="px-3.5 py-2 text-left font-normal">
              決算期 / 開示日
            </th>
            <th className="px-3.5 py-2 text-right font-normal">売上</th>
            <th className="px-3.5 py-2 text-right font-normal">営業益</th>
            <th className="px-3.5 py-2 text-right font-normal">EPS</th>
          </tr>
        </thead>
        <tbody>
          {orderedSummaries.map((summary) => (
            <tr
              key={`${summary.disclosure_no}/${summary.report_group_key}`}
              className="border-b border-border last:border-b-0"
            >
              <td className="px-3.5 py-2.5">
                <p className="tabular-nums text-foreground">
                  {formatPeriod(summary)}
                </p>
                <time
                  dateTime={summary.disclosure_date}
                  className="text-2xs tabular-nums text-muted-foreground"
                >
                  {formatDisclosureDate(summary.disclosure_date)} 開示
                </time>
              </td>
              <td className="px-3.5 py-2.5 text-right tabular-nums">
                {formatYenInHundredMillions(summary.sales)}
              </td>
              <td className="px-3.5 py-2.5 text-right tabular-nums">
                {formatYenInHundredMillions(summary.operating_profit)}
              </td>
              <td className="px-3.5 py-2.5 text-right tabular-nums">
                {formatDecimal(summary.eps)}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}

export function StockPageFinancials({
  summaries,
  summariesState,
  groups,
  groupsState,
}: {
  summaries: FinancialSummary[]
  summariesState: StockPageListState
  groups: GroupMembership[]
  groupsState: StockPageListState
}) {
  return (
    <div className="space-y-3">
      <StockPageGroups
        memberships={groups}
        state={groupsState}
        className="sm:hidden"
      />
      {summariesState === 'loading' ? (
        <p className="px-3.5 py-4 font-mono text-xs text-muted-foreground">
          決算短信の要約を読み込み中
        </p>
      ) : summariesState === 'error' ? (
        <p className="px-3.5 py-4 font-mono text-xs text-destructive">
          決算短信の要約の取得に失敗しました
        </p>
      ) : summaries.length === 0 ? (
        <p className="px-3.5 py-4 font-mono text-xs text-muted-foreground">
          決算短信の要約はありません
        </p>
      ) : (
        <>
          <FinancialSummaryTable summaries={summaries} />
          <p className="px-3.5 pb-3 font-mono text-2xs text-muted-foreground">
            決算短信の要約を開示日の新しい順に表示しています。
          </p>
        </>
      )}
    </div>
  )
}
