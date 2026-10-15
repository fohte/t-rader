import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { StockPageGroups } from '#components/stock-page/stock-page-groups'
import type { components } from '#lib/api/schema.gen'
import { createStoryRouter } from '#storybook/story-router'

type GroupMembership = components['schemas']['StockGroupMembershipResponse']

const memberships: GroupMembership[] = [
  {
    axis: { key: 'demo-axis', name: 'デモ軸' },
    group: { key: 'demo-group', name: 'デモ群' },
  },
  {
    axis: { key: 'sample-axis', name: 'サンプル軸' },
    group: { key: 'sample-group', name: 'サンプル群' },
  },
]

afterEach(cleanup)

describe('StockPageGroups', () => {
  it('それぞれの所属グループのページへリンクする', async () => {
    const router = createStoryRouter(
      () => <StockPageGroups memberships={memberships} state="ready" />,
      { paths: ['/groups/$axisKey/$groupKey'] },
    )
    render(<RouterProvider router={router} />)

    const links = await screen.findAllByRole('link')

    expect(
      links.map((link) => [link.textContent, link.getAttribute('href')]),
    ).toEqual([
      ['デモ軸 / デモ群', '/groups/demo-axis/demo-group'],
      ['サンプル軸 / サンプル群', '/groups/sample-axis/sample-group'],
    ])
  })
})
