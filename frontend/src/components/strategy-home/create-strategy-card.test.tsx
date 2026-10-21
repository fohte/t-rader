import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { CreateStrategyCard } from '#components/strategy-home/create-strategy-card'

afterEach(cleanup)

describe('CreateStrategyCard', () => {
  it('作成操作を callback に渡す', async () => {
    const onClick = vi.fn()
    const user = userEvent.setup()
    render(<CreateStrategyCard onClick={onClick} />)

    await user.click(screen.getByRole('button', { name: '新しい戦略を作る' }))

    expect(onClick.mock.calls).toEqual([[]])
  })
})
