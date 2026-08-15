import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { buildNodeProps } from '#components/graph/flow-node-props.test-helper'
import { GraphGroupNodeView } from '#components/graph/graph-group-node'

afterEach(cleanup)

describe('GraphGroupNodeView', () => {
  it('data.label を表示する', () => {
    render(
      <GraphGroupNodeView
        {...buildNodeProps({ id: 'group1', label: 'グループ1' }, 'graphGroup')}
      />,
    )
    expect(screen.getByText('グループ1')).toBeInTheDocument()
  })
})
