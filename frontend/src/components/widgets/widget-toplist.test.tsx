import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { WidgetToplist } from '#components/widgets/widget-toplist'
import type { components } from '#lib/api/schema.gen'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']

const result: WidgetQueryResponse = {
  as_of: '2099-01-01',
  rows: [
    { key: 'stock:DEMO-C', name: '架空銘柄 C', values: { change: -0.6 } },
    { key: 'stock:DEMO-A', name: '架空銘柄 A', values: { change: 2.1 } },
    { key: 'stock:DEMO-D', name: '架空銘柄 D', values: { change: -1.4 } },
    { key: 'stock:DEMO-B', name: '架空銘柄 B', values: { change: 0.8 } },
  ],
}

afterEach(cleanup)

describe('WidgetToplist', () => {
  it('指定した順序と件数で行を表示する', () => {
    render(
      <WidgetToplist
        result={result}
        viz={{ type: 'toplist', value: 'change', order: 'desc', limit: 2 }}
      />,
    )

    expect(
      screen
        .getAllByRole('listitem')
        .map((row) => row.getAttribute('aria-label')),
    ).toEqual(['1 位: 架空銘柄 A 2.1', '2 位: 架空銘柄 B 0.8'])
  })

  it('昇順を選んだ場合に小さい値から表示する', () => {
    render(
      <WidgetToplist
        result={result}
        viz={{ type: 'toplist', value: 'change', order: 'asc', limit: 2 }}
      />,
    )

    expect(
      screen
        .getAllByRole('listitem')
        .map((row) => row.getAttribute('aria-label')),
    ).toEqual(['1 位: 架空銘柄 D -1.4', '2 位: 架空銘柄 C -0.6'])
  })

  it('値が null または欠落した行を表示しない', () => {
    render(
      <WidgetToplist
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
              values: { change: 0.5 },
            },
          ],
        }}
        viz={{ type: 'toplist', value: 'change', order: 'desc', limit: 3 }}
      />,
    )

    expect(
      screen
        .getAllByRole('listitem')
        .map((row) => row.getAttribute('aria-label')),
    ).toEqual(['1 位: 架空銘柄 C 0.5'])
  })

  it('指定した値を持つ行がない場合に空状態を表示する', () => {
    render(
      <WidgetToplist
        result={{ as_of: '2099-01-01', rows: [] }}
        viz={{ type: 'toplist', value: 'change', order: 'desc', limit: 3 }}
      />,
    )

    expect(screen.getByRole('status').textContent).toBe(
      '表示できるデータがありません。',
    )
  })
})
