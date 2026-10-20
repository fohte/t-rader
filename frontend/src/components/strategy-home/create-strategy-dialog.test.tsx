import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { CreateStrategyDialog } from '#components/strategy-home/create-strategy-dialog'
import { createStoryRouter } from '#storybook/story-router'

function renderOpenDialog() {
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  const router = createStoryRouter(
    () => (
      <QueryClientProvider client={queryClient}>
        <CreateStrategyDialog open onOpenChange={() => {}} />
      </QueryClientProvider>
    ),
    { paths: ['/strategies/$id'] },
  )
  return render(<RouterProvider router={router} />)
}

afterEach(cleanup)

describe('CreateStrategyDialog', () => {
  it('名前の入力欄に汎用的な入力案内を表示する', async () => {
    renderOpenDialog()
    const nameInput = await screen.findByLabelText('戦略名 *')

    expect(nameInput.getAttribute('placeholder')).toEqual(
      '名前を入力してください',
    )
  })
})
