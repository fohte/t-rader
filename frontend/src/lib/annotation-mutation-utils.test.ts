import { describe, expect, it, vi } from 'vitest'

import {
  deleteChartAnnotation,
  saveHorizontalLineAnnotation,
} from '#lib/annotation-mutation-utils'

describe('annotation mutation utilities', () => {
  it('sends a human annotation at the placed bar without period fields', () => {
    const mutate = vi.fn()

    saveHorizontalLineAnnotation(mutate, 'demo-code', {
      target_kind: 'sample-kind',
      text: '架空の価格水準を記録する',
      timestamp: '2030-04-02T06:00:00Z',
      price: 3110,
    })

    expect(mutate.mock.calls).toEqual([
      [
        {
          body: {
            target_kind: 'sample-kind',
            target_symbol: 'demo-code',
            text: '架空の価格水準を記録する',
            timestamp: '2030-04-02T06:00:00Z',
            price: 3110,
            created_by_kind: 'human',
            status: 'approved',
          },
        },
      ],
    ])
  })

  it('sends the annotation ID in the delete API path', () => {
    const mutate = vi.fn()

    deleteChartAnnotation(mutate, 'annotation-id')

    expect(mutate.mock.calls).toEqual([
      [{ params: { path: { id: 'annotation-id' } } }],
    ])
  })
})
