import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'

import { WidgetTable } from '#components/widgets/widget-table'
import type { components } from '#lib/api/schema.gen'
import type { WidgetQueryDefinitions } from '#lib/widget-value-format'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']
type WidgetTableDefinition = Extract<
  components['schemas']['WidgetVisualizationRequest'],
  { type: 'table' }
>

const queries: WidgetQueryDefinitions = {
  value: { metric: 'close', by: 'stock' },
  change: { metric: 'change', period: '1d', by: 'stock' },
}
const display = {
  value: { label: '指標値', format: 'number' },
  change: { label: '変化率' },
} satisfies components['schemas']['WidgetQueryRequest']['display']
const viz: WidgetTableDefinition = {
  type: 'table',
  columns: ['change', 'value'],
}

function readTableOutput() {
  const table = screen.queryByRole('table', { name: 'ウィジェットの表' })
  const navigation = screen.queryByRole('navigation', { name: '表のページ' })

  return {
    headers:
      table == null
        ? null
        : Array.from(table.querySelectorAll('thead th'), (cell) =>
            cell.textContent.trim(),
          ),
    rows:
      table == null
        ? null
        : Array.from(table.querySelectorAll('tbody tr'), (row) =>
            Array.from(row.querySelectorAll('th, td'), (cell) =>
              cell.textContent.trim(),
            ),
          ),
    page: navigation?.querySelector('span')?.textContent.trim() ?? null,
    previousDisabled:
      navigation
        ?.querySelector('button[aria-label="前のページ"]')
        ?.hasAttribute('disabled') ?? null,
    nextDisabled:
      navigation
        ?.querySelector('button[aria-label="次のページ"]')
        ?.hasAttribute('disabled') ?? null,
  }
}

function renderTable(
  result: WidgetQueryResponse,
  tableViz: WidgetTableDefinition = viz,
) {
  return render(
    <WidgetTable
      display={display}
      queries={queries}
      result={result}
      viz={tableViz}
    />,
  )
}

function createTableRows(prefix: string, count: number) {
  return Array.from({ length: count }, (_, index) => {
    const number = String(index + 1).padStart(2, '0')
    return {
      key: `stock:DEMO-${number}`,
      name: `${prefix} ${number}`,
      values: { value: index + 1 },
    }
  })
}

function readEmptyTableOutput() {
  return {
    message: screen.getByRole('status').textContent,
    table: screen.queryByRole('table'),
  }
}

afterEach(cleanup)

describe('WidgetTable', () => {
  it('指定列順で対象名、整形済みの値、欠損セルを表示する', () => {
    renderTable({
      as_of: '2099-01-05',
      rows: [
        {
          key: 'stock:DEMO-A',
          name: 'サンプル対象 A',
          values: { value: 1234.56, change: 0.0125 },
        },
        {
          key: 'stock:DEMO-B',
          name: 'サンプル対象 B',
          values: { change: -0.005, value: null },
        },
      ],
    })

    expect(readTableOutput()).toEqual({
      headers: ['対象', '変化率', '指標値'],
      rows: [
        ['サンプル対象 A', '+1.25%', '1,234.56'],
        ['サンプル対象 B', '-0.50%', '—'],
      ],
      page: null,
      previousDisabled: null,
      nextDisabled: null,
    })
  })

  it('行数が多いときにページ分割し、次ページの残り行を表示する', async () => {
    const user = userEvent.setup()
    const rows = createTableRows('サンプル', 21)

    renderTable(
      { as_of: '2099-01-05', rows },
      { type: 'table', columns: ['value'] },
    )
    await user.click(screen.getByRole('button', { name: '次のページ' }))

    expect(readTableOutput()).toEqual({
      headers: ['対象', '指標値'],
      rows: [['サンプル 21', '21']],
      page: '2 / 2',
      previousDisabled: false,
      nextDisabled: true,
    })
  })

  it('結果が更新されたときに先頭ページへ戻る', async () => {
    const user = userEvent.setup()
    const firstResult = {
      as_of: '2099-01-05',
      rows: createTableRows('旧結果', 21),
    }
    const nextResult = {
      as_of: '2099-01-06',
      rows: createTableRows('新結果', 21),
    }
    const { rerender } = renderTable(firstResult, {
      type: 'table',
      columns: ['value'],
    })

    await user.click(screen.getByRole('button', { name: '次のページ' }))
    rerender(
      <WidgetTable
        display={display}
        queries={queries}
        result={nextResult}
        viz={{ type: 'table', columns: ['value'] }}
      />,
    )

    expect(readTableOutput()).toEqual({
      headers: ['対象', '指標値'],
      rows: Array.from({ length: 20 }, (_, index) => [
        `新結果 ${String(index + 1).padStart(2, '0')}`,
        String(index + 1),
      ]),
      page: '1 / 2',
      previousDisabled: true,
      nextDisabled: false,
    })
  })

  it('行がない場合に空の状態を表示する', () => {
    renderTable({ as_of: '2099-01-05', rows: [] })

    expect(readEmptyTableOutput()).toEqual({
      message: '表示できる値がありません。',
      table: null,
    })
  })
})
