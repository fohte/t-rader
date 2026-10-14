import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { StrategyRunsHeading } from '#components/strategy-shell/strategy-runs-heading'
import { createStoryRouter } from '#storybook/story-router'

afterEach(cleanup)

describe('StrategyRunsHeading', () => {
  it('戦略実行一覧の見出しから全戦略の実行履歴へ移動できる', async () => {
    const router = createStoryRouter(() => <StrategyRunsHeading />, {
      paths: ['/strategies'],
      initialPath: '/strategies',
    })

    render(<RouterProvider router={router} />)

    const heading = await screen.findByRole('heading', { level: 1 })
    const link = screen.getByRole('link')
    const output = `${heading.textContent} | ${link.textContent} | ${link.getAttribute('href') ?? ''}`
    expect(output).toBe(
      '>この戦略の実行 | 全戦略の実行 → | /strategies?strategyTab=runs',
    )
  })
})
