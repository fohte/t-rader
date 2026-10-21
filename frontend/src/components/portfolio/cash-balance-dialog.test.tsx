import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { CashBalanceDialog } from '#components/portfolio/cash-balance-dialog'

afterEach(cleanup)

describe('CashBalanceDialog', () => {
  it('空の投資可能額では保存を無効にする', () => {
    render(
      <CashBalanceDialog
        open
        initial={250000}
        initialInputValue=""
        onOpenChange={vi.fn()}
        onSave={vi.fn()}
      />,
    )

    expect(
      screen.getByRole('button', { name: '保存' }).hasAttribute('disabled'),
    ).toBe(true)
  })

  it('小数の投資可能額を円単位に丸めて保存する', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn()
    render(
      <CashBalanceDialog
        open
        initial={250000}
        initialInputValue=""
        onOpenChange={vi.fn()}
        onSave={onSave}
      />,
    )

    await user.type(screen.getByLabelText('投資可能額 (¥)'), '123.6')
    await user.click(screen.getByRole('button', { name: '保存' }))

    expect(onSave.mock.calls).toEqual([[124]])
  })
})
