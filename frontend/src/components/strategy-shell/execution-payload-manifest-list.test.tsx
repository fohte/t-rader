import { cleanup, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  ExecutionPayloadManifestList,
  type ExecutionPayloadManifestSummary,
} from '#components/strategy-shell/execution-payload-manifest-list'

const items: ExecutionPayloadManifestSummary[] = [
  {
    id: '00000000-0000-4000-8000-000000000011',
    kind: 'input',
    source: 'sample-source',
    status: 'incomplete',
    itemCount: 4,
    byteCount: 256,
    digest: null,
    incompleteReason: 'capture interrupted',
    createdAt: '2030-01-02T03:04:05Z',
  },
]

afterEach(() => {
  cleanup()
})

function readManifestListOutput(section: HTMLElement) {
  const row = within(section).getByRole('listitem')
  return {
    summary: within(section).getByText('全 5 件 · 表示 1 件').textContent,
    row: {
      details: Array.from(row.querySelectorAll('p')).map((paragraph) =>
        paragraph.textContent.replace(/\s+/g, ' ').trim(),
      ),
      buttons: within(row)
        .getAllByRole('button')
        .map((button) => button.textContent.trim()),
    },
    buttons: within(section)
      .getAllByRole('button')
      .map((button) => button.textContent.trim()),
  }
}

describe('ExecutionPayloadManifestList', () => {
  it('shows a bounded manifest summary without payload content', () => {
    render(
      <ExecutionPayloadManifestList
        state={{
          status: 'ready',
          items,
          totalCount: 5,
          nextCursor: 'sample-cursor',
          selectedPayloadId: null,
          hasPreviousPage: false,
        }}
        onSelectManifest={() => {}}
        onNextPage={() => {}}
      />,
    )
    const section = screen.getByRole('region', {
      name: '実行 payload 記録',
    })
    const expected = {
      summary: '全 5 件 · 表示 1 件',
      row: {
        details: [
          '00000000-0000-4000-8000-000000000011 · 入力 · 未完了',
          'sample-source · 4 件 · 256 bytes · 2030-01-02T03:04:05Z',
          '理由: capture interrupted',
        ],
        buttons: ['payload を選択'],
      },
      buttons: ['payload を選択', '次の記録を表示'],
    }
    expect(readManifestListOutput(section)).toEqual(expected)
  })

  it('selects a manifest that has no step reference', async () => {
    const user = userEvent.setup()
    const onSelectManifest = vi.fn()
    render(
      <ExecutionPayloadManifestList
        state={{
          status: 'ready',
          items,
          totalCount: 1,
          nextCursor: null,
          selectedPayloadId: null,
          hasPreviousPage: false,
        }}
        onSelectManifest={onSelectManifest}
      />,
    )

    await user.click(screen.getByRole('button', { name: 'payload を選択' }))

    expect(onSelectManifest.mock.calls).toEqual([
      [
        {
          id: '00000000-0000-4000-8000-000000000011',
          kind: 'input',
          source: 'sample-source',
          status: 'incomplete',
          itemCount: 4,
          byteCount: 256,
          digest: null,
          incompleteReason: 'capture interrupted',
          createdAt: '2030-01-02T03:04:05Z',
        },
      ],
    ])
  })

  it('requests the next bounded manifest page', async () => {
    const user = userEvent.setup()
    const onNextPage = vi.fn()
    render(
      <ExecutionPayloadManifestList
        state={{
          status: 'ready',
          items,
          totalCount: 5,
          nextCursor: 'sample-cursor',
          selectedPayloadId: null,
          hasPreviousPage: false,
        }}
        onNextPage={onNextPage}
      />,
    )

    await user.click(screen.getByRole('button', { name: '次の記録を表示' }))

    expect(onNextPage.mock.calls).toEqual([['sample-cursor']])
  })

  it('requests the previous bounded manifest page', async () => {
    const user = userEvent.setup()
    const onPreviousPage = vi.fn()
    render(
      <ExecutionPayloadManifestList
        state={{
          status: 'ready',
          items,
          totalCount: 5,
          nextCursor: 'sample-cursor',
          selectedPayloadId: null,
          hasPreviousPage: true,
        }}
        onPreviousPage={onPreviousPage}
      />,
    )

    await user.click(screen.getByRole('button', { name: '前の記録を表示' }))

    expect(onPreviousPage.mock.calls).toEqual([[]])
  })
})
