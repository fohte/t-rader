import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { buildNodeProps } from '#components/graph/flow-node-props.test-helper'
import { GraphGroupNodeView } from '#components/graph/graph-group-node'
import { GraphRenderContextProvider } from '#components/graph/graph-render-context'

afterEach(cleanup)

describe('GraphGroupNodeView', () => {
  it('data.label を表示する', () => {
    render(
      <GraphRenderContextProvider
        value={{
          layout: 'flow',
          maxNodeValue: 0,
          citeNumbers: new Map(),
          highlightedNodeIds: null,
        }}
      >
        <GraphGroupNodeView
          {...buildNodeProps(
            { id: 'demo-group', label: 'デモグループ' },
            'graphGroup',
          )}
        />
      </GraphRenderContextProvider>,
    )
    expect(screen.getByText('デモグループ').textContent).toBe('デモグループ')
  })
})
