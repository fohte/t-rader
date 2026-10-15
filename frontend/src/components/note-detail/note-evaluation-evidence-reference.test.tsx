import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import type { ReactNode } from 'react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { NoteEvaluationEvidenceReference } from '#components/note-detail/note-evidence-reference'

interface EvaluationRequestOptions {
  params: { path: { evaluation_id: string } }
  signal: AbortSignal
}

interface EvaluationResponse {
  response: { status: number; ok: boolean }
  data:
    | {
        id: string
        indicator_name: string
        code: string
        args: unknown
        output: unknown
        evaluated_at: string
      }
    | undefined
}

const { getEvaluation } = vi.hoisted(() => ({
  getEvaluation:
    vi.fn<
      (
        path: string,
        options: EvaluationRequestOptions,
      ) => Promise<EvaluationResponse>
    >(),
}))

vi.mock('#lib/api/client', () => ({
  fetchClient: { GET: getEvaluation },
  $api: { useQuery: vi.fn() },
}))

const queryClient = new QueryClient({
  defaultOptions: { queries: { retry: false } },
})

function QueryClientWrapper({ children }: { children: ReactNode }) {
  return (
    <QueryClientProvider client={queryClient}>{children}</QueryClientProvider>
  )
}

afterEach(() => {
  cleanup()
  queryClient.clear()
  getEvaluation.mockReset()
})

describe('NoteEvaluationEvidenceReference', () => {
  it('loads the referenced record when the numeric citation is opened', async () => {
    const user = userEvent.setup()
    const evaluationId = '00000000-0000-0000-0000-000000000101'
    getEvaluation.mockResolvedValue({
      response: { status: 200, ok: true },
      data: {
        id: evaluationId,
        indicator_name: 'デモ指標',
        code: 'return args.base + 1',
        args: { base: 41.5 },
        output: { result: 42.5 },
        evaluated_at: '2030-01-02T03:04:05Z',
      },
    })

    render(
      <NoteEvaluationEvidenceReference
        evaluationId={evaluationId}
        outputName="result"
        value="42.5"
      />,
      { wrapper: QueryClientWrapper },
    )

    const requestsBeforeOpen = getEvaluation.mock.calls.map(
      ([path, options]) => ({
        path,
        params: options.params,
      }),
    )

    await user.click(screen.getByRole('button', { name: '42.5 の根拠を開く' }))
    const heading = await screen.findByText('デモ指標 · result')

    const requestsAfterOpen = getEvaluation.mock.calls.map(
      ([path, options]) => ({
        path,
        params: options.params,
      }),
    )
    const expectedRequestHistory = JSON.stringify([
      [],
      [
        '/api/custom-indicator-evaluations/{evaluation_id}',
        { path: { evaluation_id: evaluationId } },
      ],
      'デモ指標 · result',
    ])
    const requestHistory = JSON.stringify([
      requestsBeforeOpen,
      requestsAfterOpen.flatMap(({ path, params }) => [path, params]),
      heading.textContent,
    ])
    expect(requestHistory).toBe(expectedRequestHistory)
  })

  it('shows a missing-record message for a 404 response', async () => {
    const user = userEvent.setup()
    getEvaluation.mockResolvedValue({
      response: { status: 404, ok: false },
      data: undefined,
    })

    render(
      <NoteEvaluationEvidenceReference
        evaluationId="00000000-0000-0000-0000-000000000101"
        outputName="result"
        value="42.5"
      />,
      { wrapper: QueryClientWrapper },
    )

    await user.click(screen.getByRole('button', { name: '42.5 の根拠を開く' }))

    const status = await screen.findByRole('status')
    expect(status.textContent).toBe('実行記録が見つかりません')
  })

  it('shows an error message for a non-404 response', async () => {
    const user = userEvent.setup()
    getEvaluation.mockResolvedValue({
      response: { status: 500, ok: false },
      data: undefined,
    })

    render(
      <NoteEvaluationEvidenceReference
        evaluationId="00000000-0000-0000-0000-000000000101"
        outputName="result"
        value="42.5"
      />,
      { wrapper: QueryClientWrapper },
    )

    await user.click(screen.getByRole('button', { name: '42.5 の根拠を開く' }))

    const alert = await screen.findByRole('alert')
    expect(alert.textContent).toBe('実行記録を取得できませんでした')
  })
})
