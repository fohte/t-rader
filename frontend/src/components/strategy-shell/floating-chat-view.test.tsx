import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  type FloatingChatStatus,
  FloatingChatView,
  type FloatingChatViewProps,
} from '#components/strategy-shell/floating-chat-view'

afterEach(cleanup)

// Link コンポーネントが parent route を要求するため、最低限のテストルーターを噛ませる。
async function renderInRouter(ui: React.ReactElement): Promise<void> {
  const rootRoute = createRootRoute({ component: () => ui })
  const noteRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/strategies/$id/notes/$noteId',
    component: () => null,
  })
  const router = createRouter({
    routeTree: rootRoute.addChildren([noteRoute]),
    history: createMemoryHistory({ initialEntries: ['/'] }),
  })
  render(<RouterProvider router={router} />)
  // TanStack Router は非同期にルートを解決するため初回 render を待つ。
  await waitFor(() => {
    expect(
      document.body.firstElementChild?.children.length ?? 0,
    ).toBeGreaterThan(0)
  })
}

const NOOP = (): void => {}

function makeProps(
  overrides: Partial<FloatingChatViewProps>,
): FloatingChatViewProps {
  const base: FloatingChatViewProps = {
    open: true,
    strategyId: 'S1',
    seed: null,
    input: '',
    status: { kind: 'idle' },
    notes: [],
    steps: [],
    configPhases: [],
    onOpen: NOOP,
    onClose: NOOP,
    onInputChange: NOOP,
    onSubmit: NOOP,
  }
  return { ...base, ...overrides }
}

describe('FloatingChatView', () => {
  it('閉じているとき召喚ボタンだけを表示する', async () => {
    const onOpen = vi.fn()
    await renderInRouter(
      <FloatingChatView {...makeProps({ open: false, onOpen })} />,
    )

    const trigger = screen.getByRole('button', { name: 'アナリストを呼ぶ' })
    expect(trigger).toBeInTheDocument()
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument()

    await userEvent.click(trigger)
    expect(onOpen).toHaveBeenCalledOnce()
  })

  it('送信ボタンを押すと onSubmit が呼ばれる', async () => {
    const onSubmit = vi.fn()
    await renderInRouter(
      <FloatingChatView
        {...makeProps({ input: '半導体の見立て', onSubmit })}
      />,
    )

    await userEvent.click(screen.getByRole('button', { name: '送信' }))
    expect(onSubmit).toHaveBeenCalledOnce()
  })

  it('入力するたびに onInputChange に値が伝わる', async () => {
    const onInputChange = vi.fn()
    await renderInRouter(
      <FloatingChatView {...makeProps({ input: 'a', onInputChange })} />,
    )

    await userEvent.type(screen.getByLabelText('メッセージ入力'), 'x')
    // userEvent.type は controlled input の value 増分ではなく
    // 各キーごとの最新値 (現在値 + キー) を伝える。
    expect(onInputChange).toHaveBeenCalledWith('ax')
  })

  it('submitting 中は入力と送信ボタンを無効化する', async () => {
    await renderInRouter(
      <FloatingChatView
        {...makeProps({ input: 'x', status: { kind: 'submitting' } })}
      />,
    )

    expect(screen.getByText('タスクを投入しています…')).toBeInTheDocument()
    expect(screen.getByLabelText('メッセージ入力')).toBeDisabled()
    expect(screen.getByRole('button', { name: '送信' })).toBeDisabled()
  })

  it('polling 中は現在の phase ラベルを表示する', async () => {
    const status: FloatingChatStatus = { kind: 'polling', phase: 'running' }
    await renderInRouter(
      <FloatingChatView {...makeProps({ input: 'x', status })} />,
    )

    expect(screen.getByText('running')).toBeInTheDocument()
    expect(screen.getByText('アナリストが分析中です…')).toBeInTheDocument()
    expect(screen.getByLabelText('メッセージ入力')).toBeDisabled()
  })

  it('polling 中に steps があるとフェーズ木を表示する', async () => {
    const status: FloatingChatStatus = { kind: 'polling', phase: 'running' }
    await renderInRouter(
      <FloatingChatView
        {...makeProps({
          input: 'x',
          status,
          configPhases: [
            { key: 'plan', label: '調査計画', model: 'claude-opus-4' },
            {
              key: 'investigate',
              label: '個別調査',
              model: 'deepseek-v4-flash',
            },
          ],
          steps: [
            {
              phase_key: 'plan',
              label: '調査計画',
              model: 'claude-opus-4',
              status: 'completed',
              started_at: '2026-06-26T00:00:00Z',
              finished_at: '2026-06-26T00:00:12Z',
            },
            {
              phase_key: 'investigate',
              label: '個別調査',
              model: 'deepseek-v4-flash',
              status: 'running',
              item: { title: '為替影響の再評価' },
              item_label: '為替影響の再評価',
              started_at: '2026-06-26T00:00:12Z',
            },
          ],
        })}
      />,
    )

    expect(screen.getByText('為替影響の再評価')).toBeInTheDocument()
  })

  it('completed では生成ノートへのリンクを表示する', async () => {
    await renderInRouter(
      <FloatingChatView
        {...makeProps({
          status: { kind: 'completed' },
          notes: [
            {
              id: 'N1',
              title: '今日の半導体メモ',
              updated_at: '2026-06-26T00:00:00Z',
            },
            { id: 'N2', title: '別件', updated_at: '2026-06-25T00:00:00Z' },
          ],
        })}
      />,
    )

    expect(screen.getByText('completed')).toBeInTheDocument()
    const link1 = screen.getByRole('link', { name: /今日の半導体メモ/ })
    const link2 = screen.getByRole('link', { name: /別件/ })
    expect(link1).toHaveAttribute('href', '/strategies/S1/notes/N1')
    expect(link2).toHaveAttribute('href', '/strategies/S1/notes/N2')
  })

  it('completed でもノートが空ならフォールバック文言を出す', async () => {
    await renderInRouter(
      <FloatingChatView
        {...makeProps({ status: { kind: 'completed' }, notes: [] })}
      />,
    )

    expect(
      screen.getByText('生成ノートはまだ取得できていません。'),
    ).toBeInTheDocument()
  })

  it('failed では error_summary を表示する', async () => {
    const status: FloatingChatStatus = {
      kind: 'failed',
      error_summary: 'agent crashed',
    }
    await renderInRouter(<FloatingChatView {...makeProps({ status })} />)

    expect(screen.getByText('failed')).toBeInTheDocument()
    expect(screen.getByText('タスクが失敗しました。')).toBeInTheDocument()
    expect(screen.getByText('agent crashed')).toBeInTheDocument()
  })

  it('error ではエラーメッセージを露出する', async () => {
    const status: FloatingChatStatus = {
      kind: 'error',
      message: '戦略 Agent が ready ではありません',
    }
    await renderInRouter(<FloatingChatView {...makeProps({ status })} />)

    expect(screen.getByText('error')).toBeInTheDocument()
    expect(
      screen.getByText('戦略 Agent が ready ではありません'),
    ).toBeInTheDocument()
  })

  it('戦略コンテキストが無い場合は入力を無効化する', async () => {
    await renderInRouter(
      <FloatingChatView {...makeProps({ strategyId: null })} />,
    )

    const input = screen.getByLabelText('メッセージ入力')
    expect(input).toBeDisabled()
    expect(input).toHaveAttribute('placeholder', '戦略ホームを開いてください')
  })
})
