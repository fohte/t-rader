import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  type DashboardPageState,
  DashboardPageView,
} from '#components/dashboard-pages/dashboard-page-view'
import { createStoryRouter } from '#storybook/story-router'

afterEach(cleanup)

describe('DashboardPageView', () => {
  it('日付指定時に最新を選ぶと空文字で通知する', async () => {
    const user = userEvent.setup()
    const onAsOfChange = vi.fn<(date: string) => void>()
    const state: DashboardPageState = {
      status: 'ready',
      title: 'サンプル',
      asOf: '2099-02-03',
      onAsOfChange,
      widgets: [],
    }
    const router = createStoryRouter(
      () => <DashboardPageView state={state} />,
      { paths: ['/dashboards'] },
    )

    render(<RouterProvider router={router} />)
    await user.click(await screen.findByRole('button', { name: '最新' }))

    expect(onAsOfChange.mock.calls).toEqual([['']])
  })
})
