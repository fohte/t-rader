import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import {
  cleanup,
  render,
  screen,
  waitFor,
  within,
} from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { Middleware } from 'openapi-fetch'
import type { ReactNode } from 'react'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import { SKILL_NAME_ERROR_INVALID } from '@/components/strategy-settings/skill-name'
import { SkillsTab } from '@/components/strategy-settings/skills-tab'
import { fetchClient } from '@/lib/api/client'

interface SkillStore {
  [name: string]: string
}

/**
 * /api/strategies/:id/skills* に対する HTTP を、メモリ上の SkillStore で完結させる。
 * 戦略境界 / kubeopencode reconcile は backend 側で検証されているので、
 * frontend テストでは API のレスポンス契約だけ満たせばよい。
 */
function installMiddleware(initial: SkillStore = {}) {
  const store: SkillStore = { ...initial }
  const middleware: Middleware = {
    onRequest({ request }) {
      const { url } = request
      const method = request.method.toUpperCase()

      const single = /\/api\/strategies\/[^/]+\/skills\/([^/?]+)/.exec(url)
      if (single != null) {
        const name = single[1] ?? ''
        if (method === 'PUT') {
          return request
            .clone()
            .json()
            .then((body: { content: string }) => {
              store[name] = body.content
              return new Response(JSON.stringify({ content: body.content }), {
                status: 200,
                headers: { 'content-type': 'application/json' },
              })
            })
        }
        if (method === 'DELETE') {
          // eslint-disable-next-line @typescript-eslint/no-dynamic-delete
          delete store[name]
          return new Response(null, { status: 204 })
        }
      }

      if (
        /\/api\/strategies\/[^/]+\/skills(\?|$)/.test(url) &&
        method === 'GET'
      ) {
        return new Response(JSON.stringify({ skills: { ...store } }), {
          status: 200,
          headers: { 'content-type': 'application/json' },
        })
      }

      throw new Error(`unmocked request: ${method} ${url}`)
    },
  }
  fetchClient.use(middleware)
  return {
    store,
    eject: () => {
      fetchClient.eject(middleware)
    },
  }
}

let activeMiddleware: ReturnType<typeof installMiddleware> | null = null

function setup(initial: SkillStore = {}) {
  activeMiddleware?.eject()
  activeMiddleware = installMiddleware(initial)
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>
  }
  return render(<SkillsTab strategyId="strat-1" />, { wrapper: Wrapper })
}

afterEach(() => {
  cleanup()
  activeMiddleware?.eject()
  activeMiddleware = null
  vi.restoreAllMocks()
})

beforeEach(() => {
  // jsdom には window.confirm が未実装なのでテストごとに上書きする
  vi.spyOn(window, 'confirm').mockReturnValue(true)
})

describe('SkillsTab', () => {
  it('既存 skill を一覧表示する', async () => {
    setup({ snapshot: '# snapshot\n', recap: '# recap\n' })

    const list = await screen.findByTestId('skill-list')
    await waitFor(() => {
      expect(within(list).getByText('snapshot')).toBeInTheDocument()
    })
    expect(within(list).getByText('recap')).toBeInTheDocument()
  })

  it('skill を追加すると一覧に反映され、そのまま選択される', async () => {
    const user = userEvent.setup()
    setup({})

    await screen.findByTestId('skill-list')
    await user.type(screen.getByLabelText('新しい skill'), 'snapshot')
    await user.click(screen.getByRole('button', { name: '追加' }))

    await waitFor(() => {
      const list = screen.getByTestId('skill-list')
      expect(within(list).getByText('snapshot')).toBeInTheDocument()
    })
  })

  it('skill 名が無効なら API は呼ばれずエラーメッセージが出る', async () => {
    const user = userEvent.setup()
    setup({})

    await screen.findByTestId('skill-list')

    await user.type(screen.getByLabelText('新しい skill'), 'Bad Name')
    await user.click(screen.getByRole('button', { name: '追加' }))

    expect(screen.getByTestId('new-skill-error').textContent).toBe(
      SKILL_NAME_ERROR_INVALID,
    )
    // store に書き込まれていないことで「API が呼ばれていない」ことを示す
    expect(activeMiddleware?.store).toEqual({})
  })

  it('既存と同名の skill を追加しようとするとエラーになる', async () => {
    const user = userEvent.setup()
    setup({ snapshot: '' })

    await waitFor(() => {
      expect(
        within(screen.getByTestId('skill-list')).getByText('snapshot'),
      ).toBeInTheDocument()
    })

    await user.type(screen.getByLabelText('新しい skill'), 'snapshot')
    await user.click(screen.getByRole('button', { name: '追加' }))

    expect(screen.getByTestId('new-skill-error').textContent).toBe(
      '同名の skill が既に存在します',
    )
  })

  it('削除ボタンを押すと skill が一覧から消える', async () => {
    const user = userEvent.setup()
    setup({ snapshot: '', recap: '' })

    const list = await screen.findByTestId('skill-list')
    await waitFor(() => {
      expect(within(list).getByText('snapshot')).toBeInTheDocument()
    })

    await user.click(
      within(list).getByRole('button', { name: 'skill "snapshot" を削除' }),
    )

    await waitFor(() => {
      expect(within(list).queryByText('snapshot')).toBeNull()
    })
    expect(within(list).getByText('recap')).toBeInTheDocument()
  })

  it('編集して保存すると GET でも同じ content が返る', async () => {
    const user = userEvent.setup()
    setup({ snapshot: 'old content' })

    await screen.findByTestId('skill-list')
    const textarea = await screen.findByLabelText('source')
    await waitFor(() => {
      expect(textarea).toHaveValue('old content')
    })

    await user.clear(textarea)
    await user.type(textarea, 'new content')
    await user.click(screen.getByRole('button', { name: '保存' }))

    // 保存成功 → invalidate → 再 GET 後にエディタが新 content と同期する
    await waitFor(() => {
      expect(screen.queryByTestId('dirty-indicator')).toBeNull()
    })
    expect(screen.getByLabelText('source')).toHaveValue('new content')
    expect(activeMiddleware?.store).toEqual({ snapshot: 'new content' })
  })
})
