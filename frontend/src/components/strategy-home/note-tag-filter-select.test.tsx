import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { NoteTagFilterSelect } from '#components/strategy-home/note-tag-filter-select'

const noteTags = [{ key: 'theme-x', display_name: 'テーマX' }]

afterEach(cleanup)

describe('NoteTagFilterSelect', () => {
  it.each([
    { value: undefined, expected: 'すべてのタグ' },
    { value: 'theme-x', expected: 'テーマX' },
  ])(
    '閉じた selector に選択ラベルを表示する: $value',
    ({ value, expected }) => {
      render(
        <NoteTagFilterSelect
          noteTags={noteTags}
          catalogState="loaded"
          value={value}
          onChange={() => {}}
        />,
      )

      expect(
        screen.getByRole('combobox', { name: 'タグで絞り込み' }).textContent,
      ).toEqual(expected)
    },
  )

  it('選択したタグを callback に渡す', async () => {
    const onChange = vi.fn()
    const user = userEvent.setup()
    render(
      <NoteTagFilterSelect
        noteTags={noteTags}
        catalogState="loaded"
        value={undefined}
        onChange={onChange}
      />,
    )

    await user.click(screen.getByRole('combobox', { name: 'タグで絞り込み' }))
    await user.click(await screen.findByRole('option', { name: 'テーマX' }))

    expect(onChange.mock.calls).toEqual([['theme-x']])
  }, 15_000)

  it('全タグを選ぶと絞り込みを解除する', async () => {
    const onChange = vi.fn()
    const user = userEvent.setup()
    render(
      <NoteTagFilterSelect
        noteTags={noteTags}
        catalogState="loaded"
        value="theme-x"
        onChange={onChange}
      />,
    )

    await user.click(screen.getByRole('combobox', { name: 'タグで絞り込み' }))
    await user.click(
      await screen.findByRole('option', { name: 'すべてのタグ' }),
    )

    expect(onChange.mock.calls).toEqual([[undefined]])
  })

  it('does not expose an unregistered selected tag', () => {
    render(
      <NoteTagFilterSelect
        noteTags={noteTags}
        catalogState="loaded"
        value="unlisted-tag"
        onChange={() => {}}
      />,
    )

    expect(
      screen.getByRole('combobox', { name: 'タグで絞り込み' }).textContent,
    ).toEqual('未登録のタグ')
  })
})
