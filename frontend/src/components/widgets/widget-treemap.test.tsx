import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { WidgetTreemap } from '#components/widgets'
import type { components } from '#lib/api/schema.gen'
import type { WidgetQueryDefinitions } from '#lib/widget-value-format'
import { createStoryRouter } from '#storybook/story-router'

type WidgetQueryResponse = components['schemas']['WidgetQueryResponse']
type WidgetTreemapDefinition = Extract<
  components['schemas']['WidgetVisualizationRequest'],
  { type: 'treemap' }
>

const viz: WidgetTreemapDefinition = {
  type: 'treemap',
  size: 'size',
  color: 'change',
}
const queries: WidgetQueryDefinitions = {
  change: { metric: 'change', by: 'group' },
}

afterEach(cleanup)

describe('WidgetTreemap', () => {
  it('グループ行を対応するグループページへリンクする', async () => {
    const result: WidgetQueryResponse = {
      as_of: '2099-01-01',
      rows: [
        {
          key: 'group:demo-axis/demo-group-a',
          name: 'デモグループ A',
          values: { size: 7, change: 0.015 },
        },
      ],
    }
    const router = createStoryRouter(
      () => <WidgetTreemap queries={queries} result={result} viz={viz} />,
      { paths: ['/groups/$axisKey/$groupKey'] },
    )
    render(<RouterProvider router={router} />)

    expect(
      (
        await screen.findByRole('link', {
          name: 'デモグループ A: +1.50%',
        })
      ).getAttribute('href'),
    ).toBe('/groups/demo-axis/demo-group-a')
  })

  it('上昇値と下落値に対応する配色を使う', () => {
    const result: WidgetQueryResponse = {
      as_of: '2099-01-01',
      rows: [
        {
          key: 'stock:DEMO-A',
          name: '架空銘柄 A',
          values: { size: 7, change: 0.015 },
        },
        {
          key: 'stock:DEMO-B',
          name: '架空銘柄 B',
          values: { size: 4, change: -0.008 },
        },
      ],
    }
    render(<WidgetTreemap queries={queries} result={result} viz={viz} />)

    expect(
      screen.getAllByRole('listitem').map((tile) => ({
        label: tile.getAttribute('aria-label'),
        value: Array.from(tile.querySelectorAll('span')).at(-1)?.textContent,
        backgroundColor: tile.style.backgroundColor,
        borderColor: tile.style.borderColor,
      })),
    ).toEqual([
      {
        label: '架空銘柄 A: +1.50%',
        value: '+1.50%',
        backgroundColor: 'var(--color-up-dim)',
        borderColor: 'var(--color-up)',
      },
      {
        label: '架空銘柄 B: -0.80%',
        value: '-0.80%',
        backgroundColor: 'var(--color-down-dim)',
        borderColor: 'var(--color-down)',
      },
    ])
  })

  it('銘柄行をグループページへリンクしない', () => {
    const result: WidgetQueryResponse = {
      as_of: '2099-01-01',
      rows: [
        {
          key: 'stock:DEMO-A',
          name: '架空銘柄 A',
          values: { size: 7, change: 0.015 },
        },
      ],
    }
    render(<WidgetTreemap queries={queries} result={result} viz={viz} />)

    expect(screen.queryByRole('link')).toBeNull()
  })

  it('有効なタイルがない場合に空状態を表示する', () => {
    const result: WidgetQueryResponse = {
      as_of: '2099-01-01',
      rows: [
        {
          key: 'stock:DEMO-A',
          name: '架空銘柄 A',
          values: { change: 0.012 },
        },
        {
          key: 'stock:DEMO-B',
          name: '架空銘柄 B',
          values: { size: 4, change: null },
        },
        {
          key: 'stock:DEMO-C',
          name: '架空銘柄 C',
          values: { size: 0, change: 0 },
        },
      ],
    }
    render(<WidgetTreemap queries={queries} result={result} viz={viz} />)

    expect(screen.getByRole('status').textContent).toBe(
      '表示できるデータがありません。',
    )
  })
})
