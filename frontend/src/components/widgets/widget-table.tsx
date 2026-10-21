import { useState } from 'react'

import type { components } from '#lib/api/schema.gen'
import {
  formatWidgetValue,
  type WidgetDisplayDefinitions,
  type WidgetQueryDefinitions,
} from '#lib/widget-value-format'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']
type WidgetTableDefinition = Extract<
  components['schemas']['WidgetVisualizationRequest'],
  { type: 'table' }
>

const PAGE_SIZE = 20
const EMPTY_DISPLAY: WidgetDisplayDefinitions = {}

export function WidgetTable({
  result,
  viz,
  queries,
  display,
}: {
  result: WidgetQueryResponse
  viz: WidgetTableDefinition
  queries: WidgetQueryDefinitions
  display?: WidgetDisplayDefinitions
}) {
  const displayDefinitions = display ?? EMPTY_DISPLAY
  const pageCount = Math.ceil(result.rows.length / PAGE_SIZE)
  const [pagination, setPagination] = useState(() => ({
    result,
    pageIndex: 0,
  }))
  const pageIndex = pagination.result === result ? pagination.pageIndex : 0
  const page = Math.min(pageIndex, Math.max(pageCount - 1, 0))
  const rows = result.rows.slice(page * PAGE_SIZE, (page + 1) * PAGE_SIZE)

  if (result.rows.length === 0) {
    return (
      <p
        className="border border-dashed border-border px-4 py-8 text-center font-mono text-xs text-muted-foreground"
        role="status"
      >
        表示できる値がありません。
      </p>
    )
  }

  return (
    <div className="space-y-3">
      <div className="overflow-x-auto border border-border bg-background">
        <table
          aria-label="ウィジェットの表"
          className="w-full min-w-max font-mono text-xs"
        >
          <thead className="bg-surface-strong text-2xs uppercase tracking-wider text-muted-foreground">
            <tr>
              <th className="px-3 py-2 text-left font-normal" scope="col">
                対象
              </th>
              {viz.columns.map((column) => (
                <th
                  className="whitespace-nowrap px-3 py-2 text-right font-normal"
                  key={column}
                  scope="col"
                >
                  {displayDefinitions[column]?.label ?? column}
                </th>
              ))}
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr
                className="border-t border-border hover:bg-surface-strong"
                key={row.key}
              >
                <th
                  className="px-3 py-2 text-left font-normal text-foreground"
                  scope="row"
                >
                  {row.name}
                </th>
                {viz.columns.map((column) => {
                  const value = row.values[column]
                  return (
                    <td
                      className="whitespace-nowrap px-3 py-2 text-right tabular-nums text-muted-foreground-strong"
                      key={column}
                    >
                      {typeof value === 'number'
                        ? formatWidgetValue(
                            column,
                            value,
                            queries,
                            displayDefinitions,
                          )
                        : '—'}
                    </td>
                  )
                })}
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      {pageCount > 1 ? (
        <nav
          aria-label="表のページ"
          className="flex items-center justify-end gap-3 font-mono text-xs text-muted-foreground"
        >
          <span aria-live="polite">
            {page + 1} / {pageCount}
          </span>
          <button
            aria-label="前のページ"
            className="border border-border px-2 py-1 text-foreground hover:bg-accent focus-visible:outline-2 focus-visible:outline-ring disabled:cursor-not-allowed disabled:opacity-50"
            disabled={page === 0}
            onClick={() => {
              setPagination({ result, pageIndex: page - 1 })
            }}
            type="button"
          >
            前へ
          </button>
          <button
            aria-label="次のページ"
            className="border border-border px-2 py-1 text-foreground hover:bg-accent focus-visible:outline-2 focus-visible:outline-ring disabled:cursor-not-allowed disabled:opacity-50"
            disabled={page >= pageCount - 1}
            onClick={() => {
              setPagination({ result, pageIndex: page + 1 })
            }}
            type="button"
          >
            次へ
          </button>
        </nav>
      ) : null}
    </div>
  )
}
