import type { Meta, StoryObj } from '@storybook/react-vite'
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import type { Middleware } from 'openapi-fetch'
import type { ReactNode } from 'react'
import { useEffect, useState } from 'react'
import { expect, userEvent, waitFor, within } from 'storybook/test'

import { RiskPolicyTab } from '#components/strategy-settings/risk-policy-tab'
import { fetchClient } from '#lib/api/client'

// Storybook にはグローバルな QueryClientProvider が無いため、$api.useQuery を使う
// RiskPolicyTab 用にこのファイルの story 全体でモックを用意する
function installMiddleware(maxPositionRatio: number | null, putStatus: number) {
  const middleware: Middleware = {
    onRequest({ request }) {
      const { url, method } = request
      if (/\/api\/strategies\/[^/]+\/risk-policy(\?|$)/.test(url)) {
        if (method === 'GET') {
          return new Response(
            JSON.stringify({ max_position_ratio: maxPositionRatio }),
            { status: 200, headers: { 'content-type': 'application/json' } },
          )
        }
        if (method === 'PUT') {
          const body =
            putStatus === 200
              ? JSON.stringify({ max_position_ratio: maxPositionRatio })
              : JSON.stringify({ error: '不正な値です' })
          return new Response(body, {
            status: putStatus,
            headers: { 'content-type': 'application/json' },
          })
        }
      }
      return new Response(`unmocked request: ${method} ${url}`, {
        status: 404,
      })
    },
  }
  fetchClient.use(middleware)
  return () => {
    fetchClient.eject(middleware)
  }
}

function QueryDecorator({
  maxPositionRatio,
  putStatus = 200,
  children,
}: {
  maxPositionRatio: number | null
  putStatus?: number
  children: ReactNode
}) {
  const [client] = useState(
    () => new QueryClient({ defaultOptions: { queries: { retry: false } } }),
  )
  // 子の useQuery が初回マウント時に fetch する前にモックを登録し切る必要があるため、
  // useEffect ではなく render 中に同期実行される lazy initializer で install する
  const [eject] = useState(() => installMiddleware(maxPositionRatio, putStatus))
  useEffect(() => eject, [eject])
  return <QueryClientProvider client={client}>{children}</QueryClientProvider>
}

const meta = {
  title: 'StrategySettings/RiskPolicyTab',
  component: RiskPolicyTab,
} satisfies Meta<typeof RiskPolicyTab>

export default meta
type Story = StoryObj<typeof meta>

export const WithLimit: Story = {
  args: { strategyId: 'strat-1' },
  decorators: [
    (Story) => (
      <QueryDecorator maxPositionRatio={0.15}>
        <Story />
      </QueryDecorator>
    ),
  ],
}

export const Unset: Story = {
  args: { strategyId: 'strat-1' },
  decorators: [
    (Story) => (
      <QueryDecorator maxPositionRatio={null}>
        <Story />
      </QueryDecorator>
    ),
  ],
}

export const SaveError: Story = {
  args: { strategyId: 'strat-1' },
  decorators: [
    (Story) => (
      <QueryDecorator maxPositionRatio={0.15} putStatus={400}>
        <Story />
      </QueryDecorator>
    ),
  ],
  // 初期表示は WithLimit と同一の見た目になるため、保存ボタンを押してエラー状態にしてから撮影する
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    // 初回 GET が解決してスケルトンから保存ボタンに切り替わるまで待つ
    const saveButton = await canvas.findByRole('button', { name: '保存' })
    await userEvent.click(saveButton)
    await waitFor(async () => {
      await expect(canvas.getByTestId('save-error').textContent).toBe(
        '保存に失敗しました',
      )
    })
  },
}
