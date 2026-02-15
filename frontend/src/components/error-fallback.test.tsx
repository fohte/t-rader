import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { ErrorFallback } from '@/components/error-fallback'

afterEach(cleanup)

describe('ErrorFallback', () => {
  it('エラーメッセージを表示する', () => {
    const error = new Error('テストエラー')
    render(<ErrorFallback error={error} resetErrorBoundary={vi.fn()} />)

    expect(screen.getByText('エラーが発生しました')).toBeDefined()
    expect(screen.getByText('テストエラー')).toBeDefined()
  })

  it('再試行ボタンをクリックすると resetErrorBoundary が呼ばれる', async () => {
    const resetErrorBoundary = vi.fn()
    const error = new Error('テストエラー')
    render(
      <ErrorFallback error={error} resetErrorBoundary={resetErrorBoundary} />,
    )

    await userEvent.click(screen.getByText('再試行'))

    expect(resetErrorBoundary).toHaveBeenCalledOnce()
  })
})
