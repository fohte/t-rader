import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { ExecutionOriginLinks } from '#components/execution-origin-links'
import { createStoryRouter } from '#storybook/story-router'

afterEach(cleanup)

describe('ExecutionOriginLinks', () => {
  it('links to the strategy and the run that generated the content', async () => {
    const router = createStoryRouter(
      () => (
        <ExecutionOriginLinks
          origin={{
            strategyId: 'bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
            strategyName: '架空戦略',
            taskId: 'cccccccc-cccc-4ccc-8ccc-cccccccccccc',
          }}
        />
      ),
      {
        paths: ['/strategies/$id', '/strategies/$id/runs/$taskId'],
        initialPath: '/strategies/sample-strategy',
      },
    )
    render(<RouterProvider router={router} />)

    const links = await screen.findAllByRole('link')
    expect(
      links.map((link) => ({
        text: link.textContent,
        href: link.getAttribute('href'),
      })),
    ).toEqual([
      {
        text: '架空戦略',
        href: '/strategies/bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb',
      },
      {
        text: '実行詳細 →',
        href: '/strategies/bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb/runs/cccccccc-cccc-4ccc-8ccc-cccccccccccc',
      },
    ])
  })
})
