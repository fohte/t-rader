import { cleanup, render } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { GraphScatterBackgroundView } from '#components/graph/graph-scatter-background'

afterEach(cleanup)

describe('GraphScatterBackgroundView', () => {
  it('十字の区切り線要素を含む DOM を描画する', () => {
    const { container } = render(<GraphScatterBackgroundView />)
    expect(container.innerHTML).toBe(
      '<div class="border-border relative h-full w-full rounded-md border">' +
        '<div class="bg-border absolute top-1/2 left-0 h-px w-full"></div>' +
        '<div class="bg-border absolute top-0 left-1/2 h-full w-px"></div>' +
        '</div>',
    )
  })
})
