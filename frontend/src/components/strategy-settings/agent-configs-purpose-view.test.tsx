import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ReactNode } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  AgentConfigPurposeView,
  type AgentConfigTab,
} from '#components/strategy-settings/agent-configs-purpose-view'

vi.mock('@tanstack/react-router', () => ({
  Link: ({ children }: { children: ReactNode }) => <a href="/">{children}</a>,
}))

afterEach(cleanup)

describe('AgentConfigPurposeView', () => {
  it('選択したタブを親へ通知する', async () => {
    const user = userEvent.setup()
    const onTabChange = vi.fn<(tab: AgentConfigTab) => void>()
    render(
      <AgentConfigPurposeView
        state="ready"
        purpose="sample-purpose"
        tab="agents-md"
        onTabChange={onTabChange}
      >
        <div />
      </AgentConfigPurposeView>,
    )

    await user.click(screen.getByRole('tab', { name: 'Skills' }))

    expect(onTabChange.mock.calls).toEqual([['skills']])
  })
})
