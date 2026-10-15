import {
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ComponentProps } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { ScreenerPageView } from '#components/screener/screener-page-view'
import type { components } from '#lib/api/schema.gen'
import type { SavedScreen } from '#lib/screener-search'

type ScreenerResponse = components['schemas']['ScreenerResponse']

const data: ScreenerResponse = {
  items: [],
  total_count: 0,
  as_of: '2099-06-18',
  page: 1,
}

const savedScreen: SavedScreen = {
  id: 'saved-screen-a',
  name: '試作条件',
  created_at: '2099-01-01T00:00:00Z',
  updated_at: '2099-01-02T00:00:00Z',
  filters: {
    markets: ['プライム'],
    sectors: [],
    per: { min: null, max: 12 },
    pbr: null,
    roe: null,
    dividend_yield: null,
    market_cap: null,
  },
  sort: { field: 'per', direction: 'asc' },
}

afterEach(cleanup)

describe('ScreenerPageView', () => {
  it('applies all selected screener conditions together', () => {
    const onApplyFilters = vi.fn()
    const args = {
      search: {},
      data,
      stocks: [],
      sectorOptions: [{ key: 'sector-sample', name: 'サンプル業' }],
      sectorOptionsPending: false,
      sectorOptionsError: false,
      status: 'results',
      loadMoreError: false,
      onApplyFilters,
      onSort: () => {},
      onSortOrder: () => {},
      onLoadMore: () => {},
      onRetryLoadMore: () => {},
    } satisfies ComponentProps<typeof ScreenerPageView>

    render(<ScreenerPageView {...args} />)

    const market = screen.getByRole('checkbox', { name: 'プライム' })
    const sector = screen.getByRole('checkbox', { name: 'サンプル業' })
    const applyButton = screen.getByRole('button', {
      name: 'この条件で絞り込む',
    })

    const ranges = [
      ['PER 下限', '8'],
      ['PER 上限', '15'],
      ['PBR 下限', '0.5'],
      ['PBR 上限', '2'],
      ['ROE 下限', '6'],
      ['ROE 上限', '20'],
      ['配当利回り (予) 下限', '2'],
      ['配当利回り (予) 上限', '5'],
      ['時価総額 下限', '500'],
      ['時価総額 上限', '10000'],
    ] as const
    const rangeInputs = ranges.map(
      ([label, value]) =>
        [
          screen.getByRole<HTMLInputElement>('spinbutton', { name: label }),
          value,
        ] as const,
    )

    fireEvent.click(market)
    fireEvent.click(sector)
    for (const [input, value] of rangeInputs) {
      fireEvent.change(input, { target: { value } })
    }

    fireEvent.click(applyButton)

    expect(onApplyFilters.mock.calls).toEqual([
      [
        {
          markets: ['プライム'],
          sectors: ['sector-sample'],
          min_per: 8,
          max_per: 15,
          min_pbr: 0.5,
          max_pbr: 2,
          min_roe: 6,
          max_roe: 20,
          min_dividend_yield: 2,
          max_dividend_yield: 5,
          min_market_cap: 500,
          max_market_cap: 10000,
        },
      ],
    ])
  })

  it('keeps an unapplied filter draft when sorting and paging change', async () => {
    const user = userEvent.setup()
    const args = {
      search: {},
      data,
      stocks: [],
      sectorOptions: [],
      sectorOptionsPending: false,
      sectorOptionsError: false,
      status: 'results',
      onApplyFilters: () => {},
      onSort: () => {},
      onSortOrder: () => {},
      onLoadMore: () => {},
      onRetryLoadMore: () => {},
    } satisfies ComponentProps<typeof ScreenerPageView>

    const { rerender } = render(<ScreenerPageView {...args} />)
    const perLowerBound = screen.getByRole<HTMLInputElement>('spinbutton', {
      name: 'PER 下限',
    })
    await user.type(perLowerBound, '12')
    rerender(
      <ScreenerPageView
        {...args}
        search={{ sort_by: 'per', sort_order: 'asc', page: 2 }}
      />,
    )

    expect(perLowerBound.value).toBe('12')
  })

  it('shows reversed range validation and does not apply invalid filters', async () => {
    const user = userEvent.setup()
    const onApplyFilters = vi.fn()
    const args = {
      search: {},
      data,
      stocks: [],
      sectorOptions: [],
      sectorOptionsPending: false,
      sectorOptionsError: false,
      status: 'results',
      onApplyFilters,
      onSort: () => {},
      onSortOrder: () => {},
      onLoadMore: () => {},
      onRetryLoadMore: () => {},
    } satisfies ComponentProps<typeof ScreenerPageView>

    render(<ScreenerPageView {...args} />)
    await user.type(screen.getByRole('spinbutton', { name: 'PER 下限' }), '20')
    await user.type(screen.getByRole('spinbutton', { name: 'PER 上限' }), '8')
    await user.click(screen.getByRole('button', { name: 'この条件で絞り込む' }))

    const actual = JSON.stringify({
      error: screen.getByRole('alert').textContent,
      calls: onApplyFilters.mock.calls,
    })
    const expected = JSON.stringify({
      error: '下限は上限以下にしてください。',
      calls: [],
    })
    expect(actual).toBe(expected)
  })

  it('selects a saved screen from the open menu', async () => {
    const user = userEvent.setup()
    const onSelectSavedScreen = vi.fn()
    const args = {
      search: {},
      data,
      stocks: [],
      sectorOptions: [],
      sectorOptionsPending: false,
      sectorOptionsError: false,
      status: 'results',
      savedScreens: [savedScreen],
      savedScreensMenuOpen: true,
      onSelectSavedScreen,
      onApplyFilters: () => {},
      onSort: () => {},
      onSortOrder: () => {},
      onLoadMore: () => {},
      onRetryLoadMore: () => {},
    } satisfies ComponentProps<typeof ScreenerPageView>

    render(<ScreenerPageView {...args} />)
    await user.click(screen.getByRole('button', { name: '試作条件' }))

    expect(onSelectSavedScreen.mock.calls).toEqual([[savedScreen]])
  })

  it('shows overwrite only after the URL conditions differ from the saved screen', async () => {
    const user = userEvent.setup()
    const search: ComponentProps<typeof ScreenerPageView>['search'] = {
      screen: savedScreen.id,
      markets: ['プライム'],
      max_per: 12,
      sort_by: 'per',
      sort_order: 'asc',
    }
    const args = {
      search,
      data,
      stocks: [],
      sectorOptions: [],
      sectorOptionsPending: false,
      sectorOptionsError: false,
      status: 'results',
      savedScreens: [savedScreen],
      onApplyFilters: () => {},
      onSort: () => {},
      onSortOrder: () => {},
      onLoadMore: () => {},
      onRetryLoadMore: () => {},
    } satisfies ComponentProps<typeof ScreenerPageView>
    const { container, rerender } = render(<ScreenerPageView {...args} />)
    const readSaveActions = () =>
      Array.from(container.querySelectorAll('button'))
        .filter((button) => button.getAttribute('aria-label') == null)
        .map((button) => button.textContent.trim())
        .filter((label) =>
          ['保存', '名前を付けて保存', '上書き保存'].includes(label),
        )

    await user.clear(screen.getByRole('spinbutton', { name: 'PER 上限' }))
    await user.type(screen.getByRole('spinbutton', { name: 'PER 上限' }), '10')
    const beforeApply = readSaveActions()

    rerender(<ScreenerPageView {...args} search={{ ...search, max_per: 10 }} />)
    const afterApply = readSaveActions()

    const actual = JSON.stringify({ beforeApply, afterApply })
    const expected = JSON.stringify({
      beforeApply: ['名前を付けて保存'],
      afterApply: ['上書き保存', '名前を付けて保存'],
    })
    expect(actual).toBe(expected)
  })

  it('shows a disabled loading action while saved screens are unresolved', () => {
    const args = {
      search: { screen: 'missing-screen' },
      data,
      stocks: [],
      sectorOptions: [],
      sectorOptionsPending: false,
      sectorOptionsError: false,
      status: 'results',
      savedScreensPending: true,
      onApplyFilters: () => {},
      onSort: () => {},
      onSortOrder: () => {},
      onLoadMore: () => {},
      onRetryLoadMore: () => {},
    } satisfies ComponentProps<typeof ScreenerPageView>

    const { container } = render(<ScreenerPageView {...args} />)

    const saveActions = Array.from(container.querySelectorAll('button'))
      .filter(
        (button) =>
          button.getAttribute('aria-label') == null &&
          ['保存', '読み込み中...', '名前を付けて保存', '上書き保存'].includes(
            button.textContent.trim(),
          ),
      )
      .map((button) => ({
        label: button.textContent.trim(),
        disabled: button.disabled,
      }))

    expect(saveActions).toEqual([{ label: '読み込み中...', disabled: true }])
  })

  it('saves the current filter draft with the requested name', async () => {
    const user = userEvent.setup()
    const onSaveSavedScreen = vi.fn()
    const onApplyFilters = vi.fn()
    const args = {
      search: { max_per: 12, sort_by: 'per', sort_order: 'asc' },
      data,
      stocks: [],
      sectorOptions: [],
      sectorOptionsPending: false,
      sectorOptionsError: false,
      status: 'results',
      savedScreenDialog: null,
      onSaveSavedScreen,
      onApplyFilters,
      onSort: () => {},
      onSortOrder: () => {},
      onLoadMore: () => {},
      onRetryLoadMore: () => {},
    } satisfies ComponentProps<typeof ScreenerPageView>

    const { rerender } = render(<ScreenerPageView {...args} />)
    await user.clear(screen.getByRole('spinbutton', { name: 'PER 上限' }))
    await user.type(screen.getByRole('spinbutton', { name: 'PER 上限' }), '10')
    rerender(
      <ScreenerPageView {...args} savedScreenDialog={{ type: 'save' }} />,
    )
    await user.type(screen.getByRole('textbox', { name: '条件名' }), '試作条件')
    const dialog = screen.getByRole('dialog')
    await user.click(within(dialog).getByRole('button', { name: '保存' }))

    expect(onSaveSavedScreen.mock.calls).toEqual([
      [
        '試作条件',
        { type: 'save' },
        {
          markets: undefined,
          sectors: undefined,
          min_per: undefined,
          max_per: 10,
          min_pbr: undefined,
          max_pbr: undefined,
          min_roe: undefined,
          max_roe: undefined,
          min_dividend_yield: undefined,
          max_dividend_yield: undefined,
          min_market_cap: undefined,
          max_market_cap: undefined,
        },
      ],
    ])
    expect(onApplyFilters.mock.calls).toEqual([])
  })
})
