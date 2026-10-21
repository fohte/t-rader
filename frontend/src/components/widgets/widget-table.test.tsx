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
  close: { metric: 'close', by: 'stock' },
  change: { metric: 'change', period: '1d', by: 'stock' },
  per: { metric: 'per', by: 'stock' },
  pbr: { metric: 'pbr', by: 'stock' },
}
const display = {
  close: { label: '終値', format: 'number' },
  change: { label: '騰落率' },
  per: { label: 'PER', format: 'number' },
  pbr: { label: 'PBR', format: 'number' },
} satisfies components['schemas']['WidgetQueryRequest']['display']
const viz: WidgetTableDefinition = {
  type: 'table',
  columns: ['close', 'change', 'per', 'pbr'],
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
          name: '架空銘柄 A',
          values: { pbr: 1.234, per: 12.345, change: 0.0125, close: 1234.56 },
        },
        {
          key: 'stock:DEMO-B',
          name: '架空銘柄 B',
          values: { change: -0.005, per: 0, pbr: null },
        },
      ],
    })

    expect(readTableOutput()).toEqual({
      headers: ['対象', '終値', '騰落率', 'PER', 'PBR'],
      rows: [
        ['架空銘柄 A', '1,234.56', '+1.25%', '12.35', '1.23'],
        ['架空銘柄 B', '—', '-0.50%', '0', '—'],
      ],
      page: null,
      previousDisabled: null,
      nextDisabled: null,
    })
  })

  it('行数が多いときにページ分割し、次ページの残り行を表示する', async () => {
    const user = userEvent.setup()
    const rows = Array.from({ length: 21 }, (_, index) => {
      const number = String(index + 1).padStart(2, '0')
      return {
        key: 'stock:SAMPLE-' + number,
        name: '架空銘柄 ' + number,
        values: { close: index + 1 },
      }
    })

    renderTable(
      { as_of: '2099-01-05', rows },
      { type: 'table', columns: ['close'] },
    )
    await user.click(screen.getByRole('button', { name: '次のページ' }))

    expect(readTableOutput()).toEqual({
      headers: ['対象', '終値'],
      rows: [['架空銘柄 21', '21']],
      page: '2 / 2',
      previousDisabled: false,
      nextDisabled: true,
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
