import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { NoteTagFilterSelect } from '#components/strategy-home/note-tag-filter-select'

afterEach(cleanup)

describe('NoteTagFilterSelect', () => {
  it('選択したタグを callback に渡す', async () => {
    const onChange = vi.fn()
    const user = userEvent.setup()
    render(
      <NoteTagFilterSelect
        tags={['架空タグ']}
        value={undefined}
        onChange={onChange}
      />,
    )

    await user.click(screen.getByRole('combobox', { name: 'タグで絞り込み' }))
    await user.click(await screen.findByRole('option', { name: '架空タグ' }))

    expect(onChange.mock.calls).toEqual([['架空タグ']])
  })

  it('全タグを選ぶと絞り込みを解除する', async () => {
    const onChange = vi.fn()
    const user = userEvent.setup()
    render(
      <NoteTagFilterSelect
        tags={['架空タグ']}
        value="架空タグ"
        onChange={onChange}
      />,
    )

    await user.click(screen.getByRole('combobox', { name: 'タグで絞り込み' }))
    await user.click(
      await screen.findByRole('option', { name: 'すべてのタグ' }),
    )

    expect(onChange.mock.calls).toEqual([[undefined]])
  })
})
