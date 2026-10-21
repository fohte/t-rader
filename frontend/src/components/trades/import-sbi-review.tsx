import { Checkbox } from '@fohte/ui/checkbox'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@fohte/ui/select'

import type { components } from '#lib/api/schema.gen'

type Strategy = components['schemas']['Strategy']
type SbiPreviewRow = components['schemas']['SbiPreviewRow']
type SbiPreviewResponse = components['schemas']['SbiPreviewResponse']

export type ImportSbiRow = SbiPreviewRow & {
  /** 取込時の戦略 ID。空文字は未割当 */
  strategyId: string
  /** 除外された行は INSERT 対象から外す */
  excluded: boolean
}

export function ImportSbiReview({
  rows,
  strategies,
  issues,
  bulkStrategy,
  importableCount,
  bulkSelectDefaultOpen = false,
  onBulkStrategyChange,
  onRowChange,
}: {
  rows: ImportSbiRow[]
  strategies: Strategy[]
  issues: SbiPreviewResponse['issues']
  bulkStrategy: string
  importableCount: number
  bulkSelectDefaultOpen?: boolean
  onBulkStrategyChange: (id: string) => void
  onRowChange: (
    idx: number,
    patch: Partial<Pick<ImportSbiRow, 'strategyId' | 'excluded'>>,
  ) => void
}) {
  return (
    <div className="flex flex-col gap-3">
      <div className="flex flex-wrap items-center gap-3 text-sm">
        <span className="text-muted-foreground-strong">
          全行を戦略にまとめて割当:
        </span>
        <Select
          value={bulkStrategy === '' ? null : bulkStrategy}
          onValueChange={(next) => {
            onBulkStrategyChange(next ?? '')
          }}
          defaultOpen={bulkSelectDefaultOpen}
        >
          <SelectTrigger aria-label="全行を戦略にまとめて割当">
            <SelectValue placeholder="未選択" />
          </SelectTrigger>
          <SelectContent>
            <SelectItem value={null}>未選択</SelectItem>
            {strategies.map((strategy) => (
              <SelectItem key={strategy.id} value={strategy.id}>
                {strategy.name}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <span className="text-muted-foreground">
          取込対象: {importableCount} / {rows.length}
        </span>
      </div>

      <div className="max-h-105 overflow-auto border border-border">
        <table className="w-full text-xs">
          <thead className="sticky top-0 bg-bg-tertiary text-left text-muted-foreground-strong">
            <tr>
              <th className="px-2 py-1.5">除外</th>
              <th className="px-2 py-1.5">日付</th>
              <th className="px-2 py-1.5">銘柄</th>
              <th className="px-2 py-1.5">売買</th>
              <th className="px-2 py-1.5 text-right">数量</th>
              <th className="px-2 py-1.5 text-right">単価</th>
              <th className="px-2 py-1.5 text-right">手数料</th>
              <th className="px-2 py-1.5">戦略</th>
              <th className="px-2 py-1.5">状態</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row, idx) => (
              <tr
                key={String(row.row_index) + '-' + row.symbol}
                data-duplicate={row.is_duplicate ? 'true' : 'false'}
                className="data-[duplicate=true]:bg-bg-tertiary/40 data-[duplicate=true]:text-muted-foreground"
              >
                <td className="px-2 py-1">
                  <Checkbox
                    aria-label={'行 ' + String(row.row_index) + ' を除外'}
                    checked={row.excluded}
                    onCheckedChange={(checked) => {
                      onRowChange(idx, { excluded: checked })
                    }}
                  />
                </td>
                <td className="px-2 py-1 font-mono">{row.date}</td>
                <td className="px-2 py-1">
                  <span className="font-mono">{row.symbol}</span>{' '}
                  <span className="text-muted-foreground">
                    {row.stock_name}
                  </span>
                </td>
                <td className="px-2 py-1">
                  {row.side === 'buy' ? '買' : '売'}
                </td>
                <td className="px-2 py-1 text-right font-mono">{row.qty}</td>
                <td className="px-2 py-1 text-right font-mono">{row.price}</td>
                <td className="px-2 py-1 text-right font-mono">{row.fee}</td>
                <td className="px-2 py-1">
                  <Select
                    value={row.strategyId === '' ? null : row.strategyId}
                    disabled={row.excluded}
                    onValueChange={(next) => {
                      onRowChange(idx, { strategyId: next ?? '' })
                    }}
                  >
                    <SelectTrigger
                      aria-label={'行 ' + String(row.row_index) + ' の戦略'}
                    >
                      <SelectValue placeholder="未選択" />
                    </SelectTrigger>
                    <SelectContent>
                      <SelectItem value={null}>未選択</SelectItem>
                      {strategies.map((strategy) => (
                        <SelectItem key={strategy.id} value={strategy.id}>
                          {strategy.name}
                        </SelectItem>
                      ))}
                    </SelectContent>
                  </Select>
                </td>
                <td className="px-2 py-1 text-2xs">
                  {row.is_duplicate ? '重複 (skip 推奨)' : '新規'}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>

      {issues.length > 0 && (
        <details className="text-xs">
          <summary className="cursor-pointer text-muted-foreground-strong">
            パース不能行: {issues.length}
          </summary>
          <ul className="mt-1 list-disc pl-5">
            {issues.map((issue) => (
              <li key={issue.row_index}>
                row {issue.row_index}: {issue.message}
              </li>
            ))}
          </ul>
        </details>
      )}
    </div>
  )
}
