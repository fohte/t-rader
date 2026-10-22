import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { MarkdownBodyGraphNotice } from '#components/note-detail/markdown-body-graph-notice'

afterEach(cleanup)

describe('MarkdownBodyGraphNotice', () => {
  it('calls the retry handler when the retry action is selected', () => {
    const onRetry = vi.fn()
    render(
      <MarkdownBodyGraphNotice onRetry={onRetry}>
        図を表示できませんでした
      </MarkdownBodyGraphNotice>,
    )

    fireEvent.click(screen.getByRole('button', { name: '再試行' }))

    const getNoticeOutput = () => ({
      notice: screen.getByRole('alert').textContent,
      retryCalls: onRetry.mock.calls.length,
    })
    expect(getNoticeOutput()).toEqual({
      notice: '図を表示できませんでした再試行',
      retryCalls: 1,
    })
  })
})
