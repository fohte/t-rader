import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { FilterBar } from '#components/runs/filter-bar'

afterEach(cleanup)

describe('FilterBar', () => {
  it('passes the selected filter value to the caller', async () => {
    const user = userEvent.setup()
    const onChange = vi.fn<(value: string) => void>()

    render(
      <FilterBar
        options={[{ id: 'demo-category', label: '架空分類', count: 2 }]}
        value="all"
        onChange={onChange}
        allLabel="すべて"
        allCount={3}
      />,
    )

    await user.click(screen.getByRole('button', { name: '架空分類 (2)' }))

    expect(onChange.mock.calls).toEqual([['demo-category']])
  })
})
