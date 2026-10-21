import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ComponentProps } from 'react'
import { afterEach, describe, expect, it } from 'vitest'

import { AgentConfigsIndexView } from '#components/strategy-settings/agent-configs-index-view'
import { createStoryRouter } from '#storybook/story-router'

type Props = ComponentProps<typeof AgentConfigsIndexView>

afterEach(cleanup)

function renderView(props: Props) {
  const router = createStoryRouter(() => <AgentConfigsIndexView {...props} />, {
    paths: ['/settings/agent-configs', '/settings/agent-configs/$purpose'],
    initialPath: '/settings/agent-configs',
  })
  render(<RouterProvider router={router} />)
}

describe('AgentConfigsIndexView', () => {
  it('入力、Enter、削除の操作を親へ通知する', async () => {
    const user = userEvent.setup()
    const actions: Array<
      | { type: 'purpose-change'; value: string }
      | { type: 'create' }
      | { type: 'delete'; purpose: string }
    > = []

    renderView({
      purposes: ['sample-purpose'],
      isPending: false,
      newPurpose: '',
      isCreating: false,
      createError: null,
      onPurposeChange: (value) => {
        actions.push({ type: 'purpose-change', value })
      },
      onCreate: () => {
        actions.push({ type: 'create' })
      },
      onDelete: (purpose) => {
        actions.push({ type: 'delete', purpose })
      },
    })

    const input = await screen.findByRole('textbox', { name: '新しい目的' })
    await user.type(input, 'x')
    await user.keyboard('{Enter}')
    await user.click(
      screen.getByRole('button', { name: '"sample-purpose" を削除' }),
    )

    expect(actions).toEqual([
      { type: 'purpose-change', value: 'x' },
      { type: 'create' },
      { type: 'delete', purpose: 'sample-purpose' },
    ])
  })
})
