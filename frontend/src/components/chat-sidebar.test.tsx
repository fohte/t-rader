import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { ChatSidebar } from '@/components/chat-sidebar'

afterEach(cleanup)

describe('ChatSidebar', () => {
  it('isOpen が true のときサイドバーの内容を表示する', () => {
    render(<ChatSidebar isOpen={true} onClose={vi.fn()} />)

    expect(screen.getByText('AI チャット')).toBeInTheDocument()
    expect(screen.getByText('AI チャット - 開発中')).toBeInTheDocument()
  })

  it('isOpen が false のときサイドバーの内容を表示しない', () => {
    render(<ChatSidebar isOpen={false} onClose={vi.fn()} />)

    expect(screen.queryByText('AI チャット')).not.toBeInTheDocument()
    expect(screen.queryByText('AI チャット - 開発中')).not.toBeInTheDocument()
  })

  it('閉じるボタンをクリックすると onClose が呼ばれる', async () => {
    const onClose = vi.fn()
    render(<ChatSidebar isOpen={true} onClose={onClose} />)

    await userEvent.click(screen.getByRole('button', { name: '閉じる' }))

    expect(onClose).toHaveBeenCalledOnce()
  })
})
