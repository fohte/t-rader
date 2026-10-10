import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { ChartAnnotationList } from '#components/annotations/chart-annotation-list'

describe('ChartAnnotationList', () => {
  it('selects the annotation whose row is clicked', async () => {
    const onSelectAnnotation = vi.fn()
    const user = userEvent.setup()

    render(
      <ChartAnnotationList
        annotations={[
          {
            id: 'annotation-row',
            target_kind: 'sample-kind',
            timestamp: '2025-01-03T00:00:00.000Z',
            price: null,
            status: 'unread',
            text: '注釈一覧の操作を確認するサンプルです。',
          },
        ]}
        selectedAnnotationId={null}
        onSelectAnnotation={onSelectAnnotation}
      />,
    )

    await user.click(screen.getByRole('button'))

    expect(onSelectAnnotation.mock.calls).toEqual([['annotation-row']])
  })
})
