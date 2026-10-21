import { RouterProvider } from '@tanstack/react-router'
import {
  cleanup,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it } from 'vitest'

import { StrategySwitcher } from '#components/strategy-shell/strategy-switcher'
import { createStoryRouter } from '#storybook/story-router'
import { StrategySwitcherQueryDecorator } from '#storybook/strategy-switcher-mock'

afterEach(cleanup)

function renderStrategySwitcher() {
  const router = createStoryRouter(() => <StrategySwitcher />, {
    paths: ['/strategies/$id/performance'],
    initialPath: '/strategies/sample-alpha/performance',
  })
  render(
    <StrategySwitcherQueryDecorator>
      <RouterProvider router={router} />
    </StrategySwitcherQueryDecorator>,
  )
  return router
}

function readStrategyLinks(navigation: HTMLElement) {
  return within(navigation)
    .getAllByRole('link')
    .map((link) => ({
      label: link.textContent,
      href: link.getAttribute('href'),
      current: link.getAttribute('aria-current'),
    }))
}

describe('StrategySwitcher', () => {
  it('opens a navigation list with the current strategy marked', async () => {
    const user = userEvent.setup()
    renderStrategySwitcher()

    await user.click(
      await screen.findByRole('button', { name: '戦略を切り替える' }),
    )
    const navigation = await screen.findByRole('navigation', {
      name: '戦略切り替え',
    })
    await within(navigation).findByRole('link', { name: '架空戦略 B' })

    expect(readStrategyLinks(navigation)).toEqual([
      {
        label: '架空戦略 A',
        href: '/strategies/sample-alpha/performance',
        current: 'page',
      },
      {
        label: '架空戦略 B',
        href: '/strategies/sample-beta/performance',
        current: null,
      },
    ])
  })

  it('navigates to the selected strategy and closes the popup', async () => {
    const user = userEvent.setup()
    const router = renderStrategySwitcher()
    const trigger = await screen.findByRole('button', {
      name: '戦略を切り替える',
    })

    await user.click(trigger)
    const navigation = await screen.findByRole('navigation', {
      name: '戦略切り替え',
    })
    await user.click(
      await within(navigation).findByRole('link', { name: '架空戦略 B' }),
    )

    const expectedOutput = {
      pathname: '/strategies/sample-beta/performance',
      expanded: 'false',
    }
    const readCurrentOutput = () => ({
      pathname: router.state.location.pathname,
      expanded: trigger.getAttribute('aria-expanded'),
    })
    await waitFor(() => {
      expect(readCurrentOutput()).toEqual(expectedOutput)
    })
  })
})
