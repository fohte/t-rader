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
        periods={[]}
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

  it('selects a standalone period when its bar is clicked', async () => {
    const onSelectAnnotation = vi.fn()
    const user = userEvent.setup()

    render(
      <AnnotationChartBand
        markers={[]}
        periods={[{ id: 'annotation-period', startX: 20, endX: 100 }]}
        width={120}
        bottom={24}
        selectedAnnotationId={null}
        onSelectAnnotation={onSelectAnnotation}
      />,
    )

    await user.click(
      screen.getByRole('button', { name: '期間アノテーションを選択' }),
    )

    expect(onSelectAnnotation.mock.calls).toEqual([['annotation-period']])
  })

  it('cycles through overlapping periods and points', async () => {
    const onSelectAnnotation = vi.fn()
    const user = userEvent.setup()
    const props = {
      markers: [{ id: 'annotation-point', x: 220 }],
      periods: [
        { id: 'annotation-period-a', startX: 120, endX: 310 },
        { id: 'annotation-period-b', startX: 250, endX: 420 },
      ],
      width: 480,
      bottom: 24,
      onSelectAnnotation,
    }
    const { rerender } = render(
      <AnnotationChartBand
        {...props}
        selectedAnnotationId="annotation-period-a"
      />,
    )
    const overlapButton = () =>
      screen.getByRole('button', { name: 'アノテーション 3 件を順に選択' })

    await user.click(overlapButton())
    rerender(
      <AnnotationChartBand
        {...props}
        selectedAnnotationId="annotation-point"
      />,
    )
    await user.click(overlapButton())

    expect(onSelectAnnotation.mock.calls).toEqual([
      ['annotation-point'],
      ['annotation-period-b'],
    ])
  })
})
