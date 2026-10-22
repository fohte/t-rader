import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { CiteBadge } from '#components/graph/cite-badge'

afterEach(cleanup)

describe('CiteBadge', () => {
  it('番号をアクセシブルな出典操作として表示する', () => {
    render(<CiteBadge number={3} cite="架空の出典テキスト" />)
    expect(
      screen.getByRole('button', { name: '出典 3 を表示' }).textContent,
    ).toEqual('3')
  })

  it('クリックすると cite の全文を Popover に表示する', async () => {
    const user = userEvent.setup()
    render(<CiteBadge number={1} cite="架空の出典テキスト" />)
    const citationTexts = [
      screen.queryByText('架空の出典テキスト')?.textContent ?? null,
    ]

    await user.click(screen.getByRole('button', { name: '出典 1 を表示' }))
    citationTexts.push(
      screen.queryByText('架空の出典テキスト')?.textContent ?? null,
    )

    expect(citationTexts).toEqual([null, '架空の出典テキスト'])
  })

  it('クリックを親のグラフ操作へ伝播させない', async () => {
    const user = userEvent.setup()
    const onParentClick = vi.fn()
    render(
      <div onClick={onParentClick}>
        <CiteBadge number={1} cite="架空の出典テキスト" />
      </div>,
    )

    await user.click(screen.getByRole('button', { name: '出典 1 を表示' }))
    await user.click(screen.getByText('架空の出典テキスト'))

    expect(onParentClick.mock.calls).toEqual([])
  })
})
