import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { WidgetToplist } from '#components/widgets'
import type { components } from '#lib/api/schema.gen'
import type { WidgetQueryDefinitions } from '#lib/widget-value-format'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']

const result: WidgetQueryResponse = {
  as_of: '2099-01-01',
  rows: [
    { key: 'stock:DEMO-C', name: '架空銘柄 C', values: { change: -0.006 } },
    { key: 'stock:DEMO-A', name: '架空銘柄 A', values: { change: 0.021 } },
    { key: 'stock:DEMO-D', name: '架空銘柄 D', values: { change: -0.014 } },
    { key: 'stock:DEMO-B', name: '架空銘柄 B', values: { change: 0.008 } },
  ],
}
const queries: WidgetQueryDefinitions = {
  change: { metric: 'change', by: 'stock' },
}

afterEach(cleanup)

describe('WidgetToplist', () => {
  it('指定した順序と件数で行を表示する', () => {
    render(
      <WidgetToplist
        queries={queries}
        result={result}
        viz={{ type: 'toplist', value: 'change', order: 'desc', limit: 2 }}
      />,
    )

    expect(
      screen.getAllByRole('listitem').map((row) => ({
        label: row.getAttribute('aria-label'),
        value: row.lastElementChild?.textContent ?? null,
      })),
    ).toEqual([
      { label: '1 位: 架空銘柄 A +2.10%', value: '+2.10%' },
      { label: '2 位: 架空銘柄 B +0.80%', value: '+0.80%' },
    ])
  })

  it('昇順を選んだ場合に小さい値から表示する', () => {
    render(
      <WidgetToplist
        queries={queries}
        result={result}
        viz={{ type: 'toplist', value: 'change', order: 'asc', limit: 2 }}
      />,
    )

    expect(
      screen.getAllByRole('listitem').map((row) => ({
        label: row.getAttribute('aria-label'),
        value: row.lastElementChild?.textContent ?? null,
      })),
    ).toEqual([
      { label: '1 位: 架空銘柄 D -1.40%', value: '-1.40%' },
      { label: '2 位: 架空銘柄 C -0.60%', value: '-0.60%' },
    ])
  })

  it('式の値は明示された表示形式でパーセント表示する', () => {
    render(
      <WidgetToplist
        queries={{}}
        display={{ relative: { label: 'Relative', format: 'percent' } }}
        result={{
          as_of: '2099-01-01',
          rows: [
            {
              key: 'stock:DEMO-A',
              name: '架空銘柄 A',
              values: { relative: 0.0123 },
            },
          ],
        }}
        viz={{ type: 'toplist', value: 'relative', order: 'desc', limit: 1 }}
      />,
    )

    expect(
      screen.getAllByRole('listitem').map((row) => ({
        label: row.getAttribute('aria-label'),
        value: row.lastElementChild?.textContent ?? null,
      })),
    ).toEqual([{ label: '1 位: 架空銘柄 A +1.23%', value: '+1.23%' }])
  })

  it('値が null または欠落した行を表示しない', () => {
    render(
      <WidgetToplist
        queries={queries}
        result={{
          as_of: '2099-01-01',
          rows: [
            {
              key: 'stock:DEMO-A',
              name: '架空銘柄 A',
              values: { change: null },
            },
            { key: 'stock:DEMO-B', name: '架空銘柄 B', values: {} },
            {
              key: 'stock:DEMO-C',
              name: '架空銘柄 C',
              values: { change: 0.005 },
            },
          ],
        }}
        viz={{ type: 'toplist', value: 'change', order: 'desc', limit: 3 }}
      />,
    )

    expect(
      screen.getAllByRole('listitem').map((row) => ({
        label: row.getAttribute('aria-label'),
        value: row.lastElementChild?.textContent ?? null,
      })),
    ).toEqual([{ label: '1 位: 架空銘柄 C +0.50%', value: '+0.50%' }])
  })

  it('指定した値を持つ行がない場合に空状態を表示する', () => {
    render(
      <WidgetToplist
        queries={queries}
        result={{ as_of: '2099-01-01', rows: [] }}
        viz={{ type: 'toplist', value: 'change', order: 'desc', limit: 3 }}
      />,
    )

    expect(screen.getByRole('status').textContent).toBe(
      '表示できるデータがありません。',
    )
  })
})
