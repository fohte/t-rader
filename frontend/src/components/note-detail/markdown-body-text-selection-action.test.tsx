import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { MarkdownBodyTextSelectionAction } from '#components/note-detail/markdown-body-text-selection-action'

afterEach(cleanup)

describe('MarkdownBodyTextSelectionAction', () => {
  it('passes the selection coordinates to CSS custom properties', () => {
    render(
      <MarkdownBodyTextSelectionAction
        selection={{
          text: '架空の本文',
          range: { startLine: 2, endLine: 3 },
          position: { top: 72, left: 276 },
        }}
        onStartComment={vi.fn()}
      />,
    )
    const button = screen.getByRole('button', {
      name: '選択した本文にコメント',
    })
    const position = button.parentElement

    const getActionOutput = () => ({
      left:
        position?.style.getPropertyValue(
          '--markdown-body-selection-action-left',
        ) ?? '',
      top:
        position?.style.getPropertyValue(
          '--markdown-body-selection-action-top',
        ) ?? '',
    })
    expect(getActionOutput()).toEqual({ left: '276px', top: '72px' })
  })
})
