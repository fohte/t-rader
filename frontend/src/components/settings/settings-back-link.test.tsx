import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { SettingsBackLink } from '#components/settings/settings-back-link'
import { createStoryRouter } from '#storybook/story-router'

afterEach(cleanup)

function getSettingsBackLinkOutput(link: HTMLElement) {
  return {
    href: link.getAttribute('href'),
    linkText: link.textContent,
  }
}

describe('SettingsBackLink', () => {
  it('設定画面へのリンクを表示する', async () => {
    const router = createStoryRouter(() => <SettingsBackLink />, {
      paths: ['/settings'],
    })
    render(<RouterProvider router={router} />)

    const link = await screen.findByRole('link')

    expect(getSettingsBackLinkOutput(link)).toEqual({
      href: '/settings',
      linkText: '< 設定に戻る',
    })
  })
})
