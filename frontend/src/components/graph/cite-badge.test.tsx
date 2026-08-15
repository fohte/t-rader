import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'

import { CiteBadge } from '#components/graph/cite-badge'

afterEach(cleanup)

describe('CiteBadge', () => {
  it('番号を表示する', () => {
    render(<CiteBadge number={3} cite="架空の出典テキスト" />)
    expect(screen.getByText('3')).toBeInTheDocument()
  })

  it('クリックすると Popover が開いて cite の文字列を表示する', async () => {
    const user = userEvent.setup()
    render(<CiteBadge number={1} cite="架空の出典テキスト" />)

    expect(screen.queryByText('架空の出典テキスト')).toBeNull()
    await user.click(screen.getByText('1'))
    expect(screen.getByText('架空の出典テキスト')).toBeInTheDocument()
  })
})
