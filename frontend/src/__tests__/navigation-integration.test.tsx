// jsdom には window.matchMedia が存在しないため、SidebarProvider (useIsMobile) 用にモックする
import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import {
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
  RouterProvider,
} from '@tanstack/react-router'
import {
  cleanup,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { FileText, History } from 'lucide-react'
import { afterEach, describe, expect, it } from 'vitest'

import { AppShell } from '@/components/app-shell'
import { PlaceholderPage } from '@/components/placeholder-page'

Object.defineProperty(window, 'matchMedia', {
  writable: true,
  value: (query: string) => ({
    matches: false,
    media: query,
    onchange: null,
    addListener: () => {},
    removeListener: () => {},
    addEventListener: () => {},
    removeEventListener: () => {},
    dispatchEvent: () => false,
  }),
})

afterEach(cleanup)

/**
 * テスト用ルーターを作成する。
 * 各ルートに実際のコンポーネント or マーカーテキストを割り当て、
 * AppShell 内でのナビゲーションを検証できるようにする。
 */
function createTestRouter(initialPath: string) {
  const rootRoute = createRootRoute({
    component: () => (
      <AppShell>
        <Outlet />
      </AppShell>
    ),
  })

  const indexRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/',
    component: () => <div data-testid="watchlist-page">ウォッチリスト画面</div>,
  })

  const tradesRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/trades',
    component: () => (
      <PlaceholderPage
        title="トレード履歴"
        description="売買記録の一覧と振り返りができます"
        icon={History}
      />
    ),
  })

  const notesRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/notes',
    component: () => (
      <PlaceholderPage
        title="ノート"
        description="日次・週次の振り返りや分析メモを記録できます"
        icon={FileText}
      />
    ),
  })

  const chartsRoute = createRoute({
    getParentRoute: () => rootRoute,
    path: '/charts/$instrumentId',
    component: () => <div data-testid="chart-page">チャート画面</div>,
  })

  const router = createRouter({
    routeTree: rootRoute.addChildren([
      indexRoute,
      tradesRoute,
      notesRoute,
      chartsRoute,
    ]),
    history: createMemoryHistory({ initialEntries: [initialPath] }),
  })

  return router
}

async function renderWithRouter(initialPath: string) {
  const router = createTestRouter(initialPath)
  const queryClient = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })

  render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  )

  // TanStack Router は非同期にルートを解決するため、AppShell のレンダリングを待つ
  await waitFor(() => {
    expect(screen.getByRole('heading', { name: 'T-Rader' })).toBeInTheDocument()
  })
}

/** 左サイドバー (data-slot="sidebar") の wrapper 要素を取得する */
function getNavSidebar() {
  // shadcn/ui の Sidebar は data-slot="sidebar" を持つ div 要素
  // その中に実際のコンテンツを持つ data-slot="sidebar-container" の div がある
  const sidebarEl = document.querySelector('[data-slot="sidebar"]')
  if (!(sidebarEl instanceof HTMLElement))
    throw new Error('sidebar element not found')
  return sidebarEl
}

/** サイドバー内のメニューリンクを取得する (hidden 要素も検索対象に含める) */
function getSidebarLink(name: string) {
  const sidebar = getNavSidebar()
  return within(sidebar).getByRole('link', {
    name: new RegExp(name),
    hidden: true,
  })
}

describe('ナビゲーション統合テスト', () => {
  describe('サイドバーメニューによるルート遷移', () => {
    it('ウォッチリストメニューをクリックすると / に遷移する', async () => {
      await renderWithRouter('/trades')

      await userEvent.click(getSidebarLink('ウォッチリスト'))

      await waitFor(() => {
        expect(screen.getByTestId('watchlist-page')).toBeInTheDocument()
      })
    })

    it('トレード履歴メニューをクリックすると /trades に遷移する', async () => {
      await renderWithRouter('/')

      await userEvent.click(getSidebarLink('トレード履歴'))

      await waitFor(() => {
        expect(
          screen.getByText('売買記録の一覧と振り返りができます'),
        ).toBeInTheDocument()
      })
    })

    it('ノートメニューをクリックすると /notes に遷移する', async () => {
      await renderWithRouter('/')

      await userEvent.click(getSidebarLink('ノート'))

      await waitFor(() => {
        expect(
          screen.getByText('日次・週次の振り返りや分析メモを記録できます'),
        ).toBeInTheDocument()
      })
    })
  })

  describe('サイドバーのメニューグループ構成', () => {
    it('マーケットグループにウォッチリストが表示される', async () => {
      await renderWithRouter('/')

      const sidebar = getNavSidebar()
      expect(within(sidebar).getByText('マーケット')).toBeInTheDocument()
      expect(
        within(sidebar).getByRole('link', {
          name: /ウォッチリスト/,
          hidden: true,
        }),
      ).toBeInTheDocument()
    })

    it('トレードグループにトレード履歴とノートが表示される', async () => {
      await renderWithRouter('/')

      const sidebar = getNavSidebar()
      expect(within(sidebar).getByText('トレード')).toBeInTheDocument()
      expect(
        within(sidebar).getByRole('link', {
          name: /トレード履歴/,
          hidden: true,
        }),
      ).toBeInTheDocument()
      expect(
        within(sidebar).getByRole('link', { name: /ノート/, hidden: true }),
      ).toBeInTheDocument()
    })
  })

  describe('プレースホルダーページの表示', () => {
    it('/trades でトレード履歴のプレースホルダーが AppShell 内に表示される', async () => {
      await renderWithRouter('/trades')

      // AppShell のサイドバーが存在する
      const sidebar = getNavSidebar()
      expect(within(sidebar).getByText('マーケット')).toBeInTheDocument()
      // プレースホルダーの内容が表示される
      expect(
        screen.getByText('売買記録の一覧と振り返りができます'),
      ).toBeInTheDocument()
    })

    it('/notes でノートのプレースホルダーが AppShell 内に表示される', async () => {
      await renderWithRouter('/notes')

      const sidebar = getNavSidebar()
      expect(within(sidebar).getByText('マーケット')).toBeInTheDocument()
      expect(
        screen.getByText('日次・週次の振り返りや分析メモを記録できます'),
      ).toBeInTheDocument()
    })
  })

  describe('AI チャットサイドバーの開閉', () => {
    it('ヘッダーのトグルボタンでチャットサイドバーが開閉する', async () => {
      await renderWithRouter('/')
      const chatSidebar = screen.getByTestId('chat-sidebar')

      // 初期状態: 閉じている
      expect(
        within(chatSidebar).queryByText('AI チャット'),
      ).not.toBeInTheDocument()

      // トグルボタンをクリックして開く
      const toggleButton = screen.getByRole('button', { name: 'AI チャット' })
      await userEvent.click(toggleButton)

      expect(within(chatSidebar).getByText('AI チャット')).toBeInTheDocument()
      expect(toggleButton).toHaveAttribute('aria-expanded', 'true')

      // 再度クリックして閉じる
      await userEvent.click(toggleButton)

      expect(
        within(chatSidebar).queryByText('AI チャット'),
      ).not.toBeInTheDocument()
      expect(toggleButton).toHaveAttribute('aria-expanded', 'false')
    })

    it('チャットサイドバーの閉じるボタンで閉じる', async () => {
      await renderWithRouter('/')

      // チャットを開く
      await userEvent.click(screen.getByRole('button', { name: 'AI チャット' }))
      expect(screen.getByText('AI チャット - 開発中')).toBeInTheDocument()

      // 閉じるボタンで閉じる
      await userEvent.click(screen.getByRole('button', { name: '閉じる' }))

      expect(screen.queryByText('AI チャット - 開発中')).not.toBeInTheDocument()
    })
  })

  describe('既存画面の動作確認', () => {
    it('ウォッチリスト画面 (/) が AppShell 内で表示される', async () => {
      await renderWithRouter('/')

      expect(screen.getByTestId('watchlist-page')).toBeInTheDocument()
      const sidebar = getNavSidebar()
      expect(within(sidebar).getByText('マーケット')).toBeInTheDocument()
    })

    it('チャート画面 (/charts/$instrumentId) が AppShell 内で表示される', async () => {
      await renderWithRouter('/charts/7203')

      expect(screen.getByTestId('chart-page')).toBeInTheDocument()
    })
  })

  describe('ルート間のナビゲーション動線', () => {
    it('ウォッチリスト → トレード履歴 → ノート → ウォッチリストと遷移できる', async () => {
      await renderWithRouter('/')

      // ウォッチリスト画面を確認
      expect(screen.getByTestId('watchlist-page')).toBeInTheDocument()

      // トレード履歴へ遷移
      await userEvent.click(getSidebarLink('トレード履歴'))
      await waitFor(() => {
        expect(
          screen.getByText('売買記録の一覧と振り返りができます'),
        ).toBeInTheDocument()
      })

      // ノートへ遷移
      await userEvent.click(getSidebarLink('ノート'))
      await waitFor(() => {
        expect(
          screen.getByText('日次・週次の振り返りや分析メモを記録できます'),
        ).toBeInTheDocument()
      })

      // ウォッチリストへ戻る
      await userEvent.click(getSidebarLink('ウォッチリスト'))
      await waitFor(() => {
        expect(screen.getByTestId('watchlist-page')).toBeInTheDocument()
      })
    })
  })
})
