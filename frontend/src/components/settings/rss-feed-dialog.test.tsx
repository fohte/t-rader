import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import {
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from '@testing-library/react'
import type { Middleware } from 'openapi-fetch'
import { afterEach, describe, expect, it } from 'vitest'

import { RssFeedDialog } from '#components/settings/rss-feed-dialog'
import { fetchClient } from '#lib/api/client'
import type { components } from '#lib/api/schema.gen'

type RssFeed = components['schemas']['RssFeed']

interface CapturedRequest {
  method: string
  path: string
  body: unknown
}

const feed: RssFeed = {
  id: '00000000-0000-0000-0000-000000000001',
  source: 'sample-feed',
  display_name: 'Example Feed',
  url: 'https://feeds.example.invalid/sample.xml',
  enabled: false,
  content_source: 'none',
  created_at: '2099-01-01T00:00:00Z',
  updated_at: '2099-01-01T00:00:00Z',
}

function renderDialog(initialFeed: RssFeed | null) {
  const requests: CapturedRequest[] = []
  const openChanges: boolean[] = []
  const middleware: Middleware = {
    async onRequest({ request }) {
      const body =
        request.method === 'POST' || request.method === 'PATCH'
          ? await request.clone().json()
          : null
      requests.push({
        method: request.method,
        path: new URL(request.url).pathname,
        body,
      })
      return new Response(JSON.stringify(feed), {
        status: 200,
        headers: { 'content-type': 'application/json' },
      })
    },
  }
  fetchClient.use(middleware)
  const queryClient = new QueryClient({
    defaultOptions: {
      queries: { retry: false },
      mutations: { retry: false },
    },
  })
  render(
    <QueryClientProvider client={queryClient}>
      <RssFeedDialog
        open
        onOpenChange={(open) => {
          openChanges.push(open)
        }}
        feed={initialFeed}
      />
    </QueryClientProvider>,
  )
  return {
    requests,
    openChanges,
    eject: () => {
      fetchClient.eject(middleware)
    },
  }
}

function createSubmissionResult(
  requests: CapturedRequest[],
  openChanges: boolean[],
) {
  return { requests, openChanges }
}

function editSubmissionResult(
  sourceInput: HTMLElement,
  requests: CapturedRequest[],
  openChanges: boolean[],
) {
  return {
    sourceDisabled: sourceInput.hasAttribute('disabled'),
    requests,
    openChanges,
  }
}

let ejectMiddleware: (() => void) | null = null

afterEach(() => {
  cleanup()
  ejectMiddleware?.()
  ejectMiddleware = null
})

describe('RssFeedDialog', () => {
  it('creates a feed with the values entered in the form', async () => {
    const { requests, openChanges, eject } = renderDialog(null)
    ejectMiddleware = eject

    fireEvent.change(screen.getByLabelText('source (slug, 不変) *'), {
      target: { value: 'sample-feed' },
    })
    fireEvent.change(screen.getByLabelText('表示名 *'), {
      target: { value: 'Example Feed' },
    })
    fireEvent.change(screen.getByLabelText('URL *'), {
      target: { value: 'https://feeds.example.invalid/sample.xml' },
    })
    fireEvent.click(screen.getByRole('checkbox', { name: '有効 (集約対象)' }))
    fireEvent.click(screen.getByRole('button', { name: '作成' }))

    await waitFor(() => {
      expect(createSubmissionResult(requests, openChanges)).toEqual({
        requests: [
          {
            method: 'POST',
            path: '/api/rss-feeds',
            body: {
              source: 'sample-feed',
              display_name: 'Example Feed',
              url: 'https://feeds.example.invalid/sample.xml',
              enabled: false,
            },
          },
        ],
        openChanges: [false],
      })
    })
  })

  it('updates editable feed values without changing the source', async () => {
    const { requests, openChanges, eject } = renderDialog(feed)
    ejectMiddleware = eject
    const sourceInput = screen.getByLabelText('source (slug, 不変) *')

    fireEvent.change(screen.getByLabelText('表示名 *'), {
      target: { value: 'Example Feed Updated' },
    })
    fireEvent.change(screen.getByLabelText('URL *'), {
      target: { value: 'https://feeds.example.invalid/updated.xml' },
    })
    fireEvent.click(screen.getByRole('checkbox', { name: '有効 (集約対象)' }))
    fireEvent.click(screen.getByRole('button', { name: '更新' }))

    await waitFor(() => {
      expect(editSubmissionResult(sourceInput, requests, openChanges)).toEqual({
        sourceDisabled: true,
        requests: [
          {
            method: 'PATCH',
            path: '/api/rss-feeds/00000000-0000-0000-0000-000000000001',
            body: {
              display_name: 'Example Feed Updated',
              url: 'https://feeds.example.invalid/updated.xml',
              enabled: true,
            },
          },
        ],
        openChanges: [false],
      })
    })
  })
})
