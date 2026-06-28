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

import { TriggersTab } from '@/components/strategy-settings/triggers-tab'
import { fetchClient } from '@/lib/api/client'
import type { components } from '@/lib/api/schema.gen'

type Trigger = components['schemas']['Trigger']

interface CreateBody {
  kind?: 'cron' | 'hook'
  schedule?: string | null
  hook_slug?: string | null
  event_match?: Record<string, never> | null
  prompt_template?: string
  enabled?: boolean
}

interface UpdateBody {
  schedule?: string | null
  hook_slug?: string | null
  event_match?: Record<string, never> | null
  prompt_template?: string
  enabled?: boolean
}

interface TriggerStore {
  byStrategy: Map<string, Trigger[]>
  byId: Map<string, Trigger>
  createCalls: Array<{ strategyId: string; body: CreateBody }>
  updateCalls: Array<{ triggerId: string; body: UpdateBody }>
  deleteCalls: string[]
}

function makeTrigger(overrides: Partial<Trigger> = {}): Trigger {
  return {
    trigger_id: overrides.trigger_id ?? crypto.randomUUID(),
    strategy_id: overrides.strategy_id ?? 'strat-1',
    kind: overrides.kind ?? 'cron',
    schedule: overrides.schedule ?? null,
    hook_slug: overrides.hook_slug ?? null,
    event_match: overrides.event_match ?? null,
    prompt_template: overrides.prompt_template ?? '',
    enabled: overrides.enabled ?? true,
    last_fired_at: overrides.last_fired_at ?? null,
    created_at: overrides.created_at ?? '2026-01-01T00:00:00Z',
    updated_at: overrides.updated_at ?? '2026-01-01T00:00:00Z',
  }
}

function installMiddleware(initial: Trigger[] = []) {
  const store: TriggerStore = {
    byStrategy: new Map(),
    byId: new Map(),
    createCalls: [],
    updateCalls: [],
    deleteCalls: [],
  }
  for (const t of initial) {
    const list = store.byStrategy.get(t.strategy_id) ?? []
    list.push(t)
    store.byStrategy.set(t.strategy_id, list)
    store.byId.set(t.trigger_id, t)
  }

  const middleware: Middleware = {
    async onRequest({ request }) {
      const { url } = request
      const method = request.method.toUpperCase()

      const triggerListMatch =
        /\/api\/strategies\/([^/]+)\/triggers(?:\?|$)/.exec(url)
      if (triggerListMatch != null) {
        const sid = triggerListMatch[1] ?? ''
        if (method === 'GET') {
          const list = store.byStrategy.get(sid) ?? []
          return new Response(JSON.stringify(list), {
            status: 200,
            headers: { 'content-type': 'application/json' },
          })
        }
        if (method === 'POST') {
          const body: CreateBody = await request.clone().json()
          store.createCalls.push({ strategyId: sid, body })
          const created = makeTrigger({
            strategy_id: sid,
            kind: body.kind ?? 'cron',
            schedule: body.schedule ?? null,
            hook_slug: body.hook_slug ?? null,
            event_match: body.event_match ?? null,
            prompt_template: body.prompt_template ?? '',
            enabled: body.enabled ?? true,
          })
          const list = store.byStrategy.get(sid) ?? []
          list.push(created)
          store.byStrategy.set(sid, list)
          store.byId.set(created.trigger_id, created)
          return new Response(JSON.stringify(created), {
            status: 201,
            headers: { 'content-type': 'application/json' },
          })
        }
      }

      const singleMatch = /\/api\/triggers\/([^/?]+)/.exec(url)
      if (singleMatch != null) {
        const tid = singleMatch[1] ?? ''
        const current = store.byId.get(tid)
        if (current == null) {
          return new Response(JSON.stringify({ error: 'not found' }), {
            status: 404,
            headers: { 'content-type': 'application/json' },
          })
        }
        if (method === 'PUT') {
          const body: UpdateBody = await request.clone().json()
          store.updateCalls.push({ triggerId: tid, body })
          const updated: Trigger = {
            ...current,
            schedule:
              'schedule' in body ? (body.schedule ?? null) : current.schedule,
            hook_slug:
              'hook_slug' in body
                ? (body.hook_slug ?? null)
                : current.hook_slug,
            event_match:
              'event_match' in body
                ? (body.event_match ?? null)
                : current.event_match,
            prompt_template:
              'prompt_template' in body
                ? (body.prompt_template ?? '')
                : current.prompt_template,
            enabled:
              'enabled' in body ? (body.enabled ?? true) : current.enabled,
          }
          store.byId.set(tid, updated)
          const list = store.byStrategy.get(updated.strategy_id) ?? []
          const idx = list.findIndex((t) => t.trigger_id === tid)
          if (idx >= 0) list[idx] = updated
          store.byStrategy.set(updated.strategy_id, list)
          return new Response(JSON.stringify(updated), {
            status: 200,
            headers: { 'content-type': 'application/json' },
          })
        }
        if (method === 'DELETE') {
          store.deleteCalls.push(tid)
          store.byId.delete(tid)
          const list = store.byStrategy.get(current.strategy_id) ?? []
          store.byStrategy.set(
            current.strategy_id,
            list.filter((t) => t.trigger_id !== tid),
          )
          return new Response(null, { status: 204 })
        }
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

function setup(initial: Trigger[] = []) {
  activeMiddleware?.eject()
  activeMiddleware = installMiddleware(initial)
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false } },
  })
  function Wrapper({ children }: { children: ReactNode }) {
    return <QueryClientProvider client={client}>{children}</QueryClientProvider>
  }
  return render(<TriggersTab strategyId="strat-1" />, { wrapper: Wrapper })
}

afterEach(() => {
  cleanup()
  activeMiddleware?.eject()
  activeMiddleware = null
  vi.restoreAllMocks()
})

beforeEach(() => {
  vi.spyOn(window, 'confirm').mockReturnValue(true)
})

describe('TriggersTab', () => {
  it('既存 trigger を一覧表示する', async () => {
    setup([
      makeTrigger({
        trigger_id: 't-cron',
        kind: 'cron',
        schedule: '0 9 * * 1-5',
        prompt_template: 'morning briefing',
      }),
      makeTrigger({
        trigger_id: 't-hook',
        kind: 'hook',
        hook_slug: 'tv-alert',
        prompt_template: 'hook fired',
      }),
    ])

    const list = await screen.findByTestId('trigger-list')
    await waitFor(() => {
      expect(within(list).getByText('0 9 * * 1-5')).toBeInTheDocument()
    })
    expect(within(list).getByText('tv-alert')).toBeInTheDocument()
  })

  it('初期表示で先頭の trigger を選択しフォームに反映する', async () => {
    setup([
      makeTrigger({
        trigger_id: 't-cron',
        kind: 'cron',
        schedule: '0 9 * * 1-5',
        prompt_template: 'morning briefing',
      }),
    ])

    const promptInput = await screen.findByLabelText('prompt_template')
    await waitFor(() => {
      expect(promptInput).toHaveValue('morning briefing')
    })
  })

  it('cron trigger を作成すると一覧に追加され、API に正しい body が送られる', async () => {
    const user = userEvent.setup()
    setup([])

    await screen.findByTestId('trigger-list')
    await user.click(screen.getByRole('button', { name: '+ 新しい trigger' }))

    await user.type(
      screen.getByLabelText('schedule (cron 式 UTC)'),
      '0 9 * * 1-5',
    )
    await user.type(
      screen.getByLabelText('prompt_template'),
      'morning briefing',
    )
    await user.click(screen.getByRole('button', { name: '作成' }))

    await waitFor(() => {
      expect(
        within(screen.getByTestId('trigger-list')).getByText('0 9 * * 1-5'),
      ).toBeInTheDocument()
    })
    expect(activeMiddleware?.store.createCalls).toEqual([
      {
        strategyId: 'strat-1',
        body: {
          kind: 'cron',
          schedule: '0 9 * * 1-5',
          hook_slug: null,
          event_match: null,
          prompt_template: 'morning briefing',
          enabled: true,
        },
      },
    ])
  })

  it('hook trigger 作成時に event_match の JSON が不正だとエラーになり API は呼ばれない', async () => {
    const user = userEvent.setup()
    setup([])

    await screen.findByTestId('trigger-list')
    await user.click(screen.getByRole('button', { name: '+ 新しい trigger' }))

    await user.selectOptions(screen.getByLabelText('kind'), 'hook')
    await user.type(
      screen.getByLabelText('hook_slug (POST /api/hooks/:slug)'),
      'x',
    )
    await user.type(
      screen.getByLabelText('event_match (JSON、空欄なら無条件)'),
      '{{not-json',
    )
    await user.type(screen.getByLabelText('prompt_template'), 'p')
    await user.click(screen.getByRole('button', { name: '作成' }))

    expect(screen.getByTestId('trigger-form-error').textContent).toBe(
      'event_match の JSON が不正です',
    )
    expect(activeMiddleware?.store.createCalls).toEqual([])
  })

  it('cron で schedule が空だと validation エラーになり API は呼ばれない', async () => {
    const user = userEvent.setup()
    setup([])

    await screen.findByTestId('trigger-list')
    await user.click(screen.getByRole('button', { name: '+ 新しい trigger' }))
    await user.type(screen.getByLabelText('prompt_template'), 'x')
    await user.click(screen.getByRole('button', { name: '作成' }))

    expect(screen.getByTestId('trigger-form-error').textContent).toBe(
      'schedule (cron 式) は必須です',
    )
    expect(activeMiddleware?.store.createCalls).toEqual([])
  })

  it('既存 trigger を編集して保存すると PUT が送られる', async () => {
    const user = userEvent.setup()
    setup([
      makeTrigger({
        trigger_id: 't-cron',
        kind: 'cron',
        schedule: '0 9 * * 1-5',
        prompt_template: 'old',
      }),
    ])

    const promptInput = await screen.findByLabelText('prompt_template')
    await waitFor(() => {
      expect(promptInput).toHaveValue('old')
    })

    await user.clear(promptInput)
    await user.type(promptInput, 'new')
    await user.click(screen.getByRole('button', { name: '保存' }))

    await waitFor(() => {
      expect(activeMiddleware?.store.updateCalls).toEqual([
        {
          triggerId: 't-cron',
          body: {
            prompt_template: 'new',
            enabled: true,
            event_match: null,
            schedule: '0 9 * * 1-5',
          },
        },
      ])
    })
  })

  it('削除ボタンを押すと DELETE が送られ一覧から消える', async () => {
    const user = userEvent.setup()
    setup([
      makeTrigger({
        trigger_id: 't-cron',
        kind: 'cron',
        schedule: '0 9 * * 1-5',
      }),
      makeTrigger({
        trigger_id: 't-hook',
        kind: 'hook',
        hook_slug: 'tv-alert',
      }),
    ])

    const list = await screen.findByTestId('trigger-list')
    await waitFor(() => {
      expect(within(list).getByText('0 9 * * 1-5')).toBeInTheDocument()
    })
    await user.click(
      within(list).getByRole('button', {
        name: 'trigger cron 0 9 * * 1-5 を削除',
      }),
    )

    await waitFor(() => {
      expect(within(list).queryByText('0 9 * * 1-5')).toBeNull()
    })
    expect(activeMiddleware?.store.deleteCalls).toEqual(['t-cron'])
  })

  it('enable トグルを切り替えると PUT { enabled } が送られる', async () => {
    const user = userEvent.setup()
    setup([
      makeTrigger({
        trigger_id: 't-cron',
        kind: 'cron',
        schedule: '0 9 * * 1-5',
        enabled: true,
      }),
    ])

    const toggle = await screen.findByRole('checkbox', {
      name: 'trigger cron 0 9 * * 1-5 の有効化',
    })
    await waitFor(() => {
      expect(toggle).toBeChecked()
    })

    await user.click(toggle)

    await waitFor(() => {
      expect(activeMiddleware?.store.updateCalls).toEqual([
        { triggerId: 't-cron', body: { enabled: false } },
      ])
    })
    await waitFor(() => {
      expect(
        screen.getByRole('checkbox', {
          name: 'trigger cron 0 9 * * 1-5 の有効化',
        }),
      ).not.toBeChecked()
    })
  })
})
