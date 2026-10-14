import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  RouterProvider,
} from '@tanstack/react-router'
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import type { ChatContext } from '#components/strategy-shell/floating-chat-store'
import {
  type FloatingChatStatus,
  FloatingChatView,
  type FloatingChatViewProps,
} from '#components/strategy-shell/floating-chat-view'

afterEach(cleanup)

// Link コンポーネントが parent route を要求するため、最低限のテストルーターを噛ませる。
async function renderInRouter(ui: React.ReactElement): Promise<HTMLElement> {
  const rootRoute = createRootRoute({ component: () => ui })
  const noteRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/notes/$noteId',
    component: () => null,
  })
  const runRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/strategies/$id/runs/$taskId',
    component: () => null,
  })
  const router = createRouter({
    routeTree: rootRoute.addChildren([noteRoute, runRoute]),
    history: createMemoryHistory({ initialEntries: ['/'] }),
  })
  const rendered = render(<RouterProvider router={router} />)
  // TanStack Router は非同期にルートを解決するため初回 render を待つ。
  await waitFor(() => {
    expect(
      document.body.firstElementChild?.children.length ?? 0,
    ).toBeGreaterThan(0)
  })
  return rendered.container
}

const NOOP = (): void => {}

function makeProps(
  overrides: Partial<FloatingChatViewProps>,
): FloatingChatViewProps {
  const base: FloatingChatViewProps = {
    open: true,
    strategyId: 'S1',
    taskStrategyId: null,
    strategies: [
      { id: 'S1', name: '例示戦略 A' },
      { id: 'S2', name: '例示戦略 B' },
    ],
    submittedPrompt: null,
    seed: null,
    input: '',
    resultText: null,
    status: { kind: 'idle' },
    notes: [],
    purposes: [],
    selectedPurpose: '',
    currentTaskId: null,
    context: null,
    onOpen: NOOP,
    onClose: NOOP,
    onInputChange: NOOP,
    onStrategyChange: NOOP,
    onPurposeChange: NOOP,
    onRemoveContext: NOOP,
    onSubmit: NOOP,
  }
  return { ...base, ...overrides }
}

function readClosedPanelOutput(openCalls: number) {
  return {
    triggerLabel: screen
      .getByRole('button', { name: 'アナリストを呼ぶ' })
      .getAttribute('aria-label'),
    dialogVisible: screen.queryByRole('dialog') != null,
    onOpenCalls: openCalls,
  }
}

function readComposerOutput() {
  const input = screen.getByLabelText('メッセージ入力')
  return {
    inputDisabled: input.hasAttribute('disabled'),
    placeholder: input.getAttribute('placeholder'),
    submitDisabled: screen
      .getByRole('button', { name: '送信' })
      .hasAttribute('disabled'),
    strategyValue: screen.getByLabelText<HTMLSelectElement>('戦略').value,
  }
}

function readContextChipOutput(label: string, removeCalls: number) {
  return {
    chipLabel: screen.getByText(label).textContent,
    removeCalls,
  }
}

function readCompletedOutput(container: HTMLElement) {
  return {
    messages: [
      ...container.querySelectorAll<HTMLElement>('[data-message-role]'),
    ].map((message) => ({
      role: message.dataset.messageRole,
      text:
        message.dataset.messageRole === 'user' || message.matches('p')
          ? message.textContent
          : null,
      paragraphs: [...message.querySelectorAll('p')].map(
        (paragraph) => paragraph.textContent,
      ),
      strong: [...message.querySelectorAll('strong')].map(
        (element) => element.textContent,
      ),
    })),
    noteLinks: [
      ...container.querySelectorAll<HTMLAnchorElement>('a[href^="/notes/"]'),
    ].map((link) => ({
      href: link.getAttribute('href'),
      label: link.firstElementChild?.textContent,
    })),
    noteFallbackVisible: [...container.querySelectorAll('p')].some(
      (paragraph) =>
        paragraph.textContent === '生成ノートはまだ取得できていません。',
    ),
  }
}

describe('FloatingChatView', () => {
  it('閉じているとき召喚ボタンだけを表示する', async () => {
    const onOpen = vi.fn()
    await renderInRouter(
      <FloatingChatView {...makeProps({ open: false, onOpen })} />,
    )

    const trigger = screen.getByRole('button', { name: 'アナリストを呼ぶ' })
    fireEvent.click(trigger)

    expect(readClosedPanelOutput(onOpen.mock.calls.length)).toEqual({
      triggerLabel: 'アナリストを呼ぶ',
      dialogVisible: false,
      onOpenCalls: 1,
    })
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

  it('文脈チップに種類と表示名を出し、外す操作を通知する', async () => {
    const onRemoveContext = vi.fn()
    const context: ChatContext = {
      kind: 'stock',
      id: 'FIC-01',
      label: '架空銘柄',
    }
    await renderInRouter(
      <FloatingChatView {...makeProps({ context, onRemoveContext })} />,
    )

    await userEvent.click(screen.getByRole('button', { name: '文脈を外す' }))

    expect(
      readContextChipOutput(
        '銘柄: 架空銘柄',
        onRemoveContext.mock.calls.length,
      ),
    ).toEqual({
      chipLabel: '銘柄: 架空銘柄',
      removeCalls: 1,
    })
  })

  it('表示名が無いときは文脈 ID を表示する', async () => {
    const context: ChatContext = { kind: 'stock', id: 'FIC-01' }
    await renderInRouter(<FloatingChatView {...makeProps({ context })} />)

    expect(screen.getByText('銘柄: FIC-01').textContent).toBe('銘柄: FIC-01')
  })

  it('文脈が無いときは削除ボタンを表示しない', async () => {
    await renderInRouter(<FloatingChatView {...makeProps({ context: null })} />)

    expect(screen.queryByRole('button', { name: '文脈を外す' })).toBeNull()
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

  it('選んだ戦略 ID を onStrategyChange に渡す', async () => {
    const onStrategyChange = vi.fn()
    await renderInRouter(
      <FloatingChatView
        {...makeProps({ strategyId: null, onStrategyChange })}
      />,
    )

    await userEvent.selectOptions(screen.getByLabelText('戦略'), 'S2')

    expect(onStrategyChange.mock.calls).toEqual([['S2']])
  })

  it('戦略未選択でも入力できるが送信できない', async () => {
    await renderInRouter(
      <FloatingChatView
        {...makeProps({ strategyId: null, input: '分析依頼' })}
      />,
    )

    expect(readComposerOutput()).toEqual({
      inputDisabled: false,
      placeholder: 'プロンプトを入力',
      submitDisabled: true,
      strategyValue: '',
    })
  })

  it('submitting 中は入力と送信ボタンを無効化する', async () => {
    await renderInRouter(
      <FloatingChatView
        {...makeProps({ input: 'x', status: { kind: 'submitting' } })}
      />,
    )

    expect(screen.getByText('タスクを投入しています…')).toBeInTheDocument()
    expect(screen.getByLabelText('実行目的')).toBeDisabled()
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

  it('currentTaskId があれば実行詳細へのリンクを表示する', async () => {
    const status: FloatingChatStatus = { kind: 'polling', phase: 'running' }
    await renderInRouter(
      <FloatingChatView
        {...makeProps({
          input: 'x',
          status,
          currentTaskId: 'example-task',
          taskStrategyId: 'S2',
        })}
      />,
    )

    const link = screen.getByRole('link', { name: /実行の詳細を見る/ })
    expect(link).toHaveAttribute('href', '/strategies/S2/runs/example-task')
  })

  it('currentTaskId が無ければ実行詳細へのリンクを出さない', async () => {
    await renderInRouter(<FloatingChatView {...makeProps({})} />)

    expect(
      screen.queryByRole('link', { name: /実行の詳細を見る/ }),
    ).not.toBeInTheDocument()
  })

  it('完了時にプロンプトと Markdown 形式の返信を表示する', async () => {
    const container = await renderInRouter(
      <FloatingChatView
        {...makeProps({
          status: { kind: 'completed' },
          submittedPrompt: '架空銘柄の変化をまとめて',
          resultText: '前回との差分です。\n\n**次回を確認します。**',
          taskStrategyId: 'S1',
          notes: [
            {
              id: 'example-note-a',
              title: '架空銘柄の分析メモ',
              updated_at: '2026-06-26T00:00:00Z',
            },
          ],
        })}
      />,
    )

    expect(readCompletedOutput(container)).toEqual({
      messages: [
        {
          role: 'user',
          text: '架空銘柄の変化をまとめて',
          paragraphs: [],
          strong: [],
        },
        {
          role: 'assistant',
          text: null,
          paragraphs: ['前回との差分です。', '次回を確認します。'],
          strong: ['次回を確認します。'],
        },
      ],
      noteLinks: [
        {
          href: '/notes/example-note-a',
          label: '架空銘柄の分析メモ',
        },
      ],
      noteFallbackVisible: false,
    })
  })

  it('completed で返信とノートが空なら両方のフォールバックを表示する', async () => {
    const container = await renderInRouter(
      <FloatingChatView
        {...makeProps({
          status: { kind: 'completed' },
          resultText: null,
          notes: [],
        })}
      />,
    )

    expect(readCompletedOutput(container)).toEqual({
      messages: [
        {
          role: 'assistant',
          text: '返信本文はありません。',
          paragraphs: [],
          strong: [],
        },
      ],
      noteLinks: [],
      noteFallbackVisible: true,
    })
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
})
