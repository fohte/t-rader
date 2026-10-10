import { render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { describe, expect, it, vi } from 'vitest'

import { AnnotationChartBand } from '#components/annotations/annotation-chart-band'

describe('AnnotationChartBand', () => {
  it('selects the next annotation when a grouped marker is clicked', async () => {
    const onSelectAnnotation = vi.fn()
    const user = userEvent.setup()

    render(
      <AnnotationChartBand
        markers={[
          { id: 'annotation-a', x: 10 },
          { id: 'annotation-b', x: 18 },
        ]}
        width={120}
        bottom={24}
        selectedAnnotationId="annotation-a"
        onSelectAnnotation={onSelectAnnotation}
      />,
    )

    await user.click(
      screen.getByRole('button', {
        name: 'アノテーション 2 件を順に選択',
      }),
    )

    expect(onSelectAnnotation.mock.calls).toEqual([['annotation-b']])
  })
})
