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
            created_by_kind: 'llm',
            target_kind: 'sample-kind',
            timestamp: '2025-01-03T00:00:00.000Z',
            price: null,
            price_start: null,
            status: 'unread',
            text: '注釈一覧の操作を確認するサンプルです。',
            timestamp_start: null,
          },
        ]}
        selectedAnnotationId={null}
        onSelectAnnotation={onSelectAnnotation}
      />,
    )

    await user.click(screen.getByRole('button'))

    expect(onSelectAnnotation.mock.calls).toEqual([['annotation-row']])
  })

  it('shows the start and end date for period annotations', () => {
    render(
      <ChartAnnotationList
        annotations={[
          {
            id: 'annotation-period',
            created_by_kind: 'llm',
            target_kind: 'sample-kind',
            timestamp_start: '2025-01-02T00:00:00.000Z',
            timestamp: '2025-01-04T00:00:00.000Z',
            price: null,
            price_start: null,
            status: 'unread',
            text: '期間表示を確認するサンプルです。',
          },
        ]}
        selectedAnnotationId={null}
        onSelectAnnotation={() => undefined}
      />,
    )

    expect(screen.getByText('1月2日 ～ 1月4日').textContent).toBe(
      '1月2日 ～ 1月4日',
    )
  })

  it('deletes the selected human annotation', async () => {
    const onDeleteAnnotation = vi.fn()
    const user = userEvent.setup()

    render(
      <ChartAnnotationList
        annotations={[
          {
            id: 'annotation-human',
            created_by_kind: 'human',
            target_kind: 'sample-level',
            timestamp: '2025-01-03T00:00:00.000Z',
            timestamp_start: null,
            price: 1250,
            price_start: null,
            status: 'approved',
            text: '保存した水平線です。',
          },
          {
            id: 'annotation-agent',
            created_by_kind: 'llm',
            target_kind: 'sample-signal',
            timestamp: '2025-01-02T00:00:00.000Z',
            timestamp_start: null,
            price: null,
            price_start: null,
            status: 'unread',
            text: '確認する注釈です。',
          },
        ]}
        selectedAnnotationId="annotation-human"
        onSelectAnnotation={() => undefined}
        onDeleteAnnotation={onDeleteAnnotation}
      />,
    )

    await user.click(
      screen.getByRole('button', {
        name: 'sample-level のアノテーションを削除',
      }),
    )

    const readDeletedAnnotationState = () => ({
      owner: screen.getByText('自分').textContent,
      deleteButton: screen.getByRole('button', {
        name: 'sample-level のアノテーションを削除',
      }).textContent,
      calls: onDeleteAnnotation.mock.calls,
    })

    expect(readDeletedAnnotationState()).toEqual({
      owner: '自分',
      deleteButton: '削除',
      calls: [['annotation-human']],
    })
  })
})
