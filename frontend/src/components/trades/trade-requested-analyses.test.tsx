import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import {
  TradeRequestedAnalyses,
  type TradeRequestedAnalysis,
} from '#components/trades/trade-requested-analyses'
import { createStoryRouter } from '#storybook/story-router'

afterEach(cleanup)

function renderAnalyses(
  analyses: TradeRequestedAnalysis[],
  state: 'loading' | 'error' | 'success',
) {
  const router = createStoryRouter(
    () => <TradeRequestedAnalyses analyses={analyses} state={state} />,
    { paths: ['/notes/$noteId', '/strategies/$id/runs/$taskId'] },
  )
  return render(<RouterProvider router={router} />)
}

function readAnalysisNotesOutput(row: Element) {
  return {
    noteLinks: Array.from(row.querySelectorAll('a[href^="/notes/"]')).map(
      (link) => ({
        label: link.textContent,
        href: link.getAttribute('href'),
      }),
    ),
    status: row.querySelector('[role="status"]')?.textContent ?? null,
    alert: row.querySelector('[role="alert"]')?.textContent ?? null,
    hasNoNotesMessage: row.textContent.includes('ノートはありません'),
  }
}

function readAnalysesOutput(section: HTMLElement) {
  return {
    heading: section.querySelector('h2')?.textContent,
    rows: Array.from(section.querySelectorAll('li')).map((row) => {
      const runDetail = row.querySelector('a[href^="/strategies/"]')
      return {
        prompt: row.querySelector('p')?.textContent,
        phase: row.querySelector('time')?.previousElementSibling?.textContent,
        date: row.querySelector('time')?.textContent,
        notes: readAnalysisNotesOutput(row),
        runDetail:
          runDetail == null
            ? null
            : {
                label: runDetail.textContent,
                href: runDetail.getAttribute('href'),
              },
      }
    }),
  }
}

function readSectionStateOutput(section: HTMLElement) {
  return {
    busy: section.getAttribute('aria-busy'),
    text: section.textContent.replace(/\s+/g, ' ').trim(),
    status: section.querySelector('[role="status"]')?.textContent ?? null,
    alert: section.querySelector('[role="alert"]')?.textContent ?? null,
  }
}

describe('TradeRequestedAnalyses', () => {
  it('shows each analysis status, note link, and run detail link', async () => {
    const analyses: TradeRequestedAnalysis[] = [
      {
        strategyId: 'fake-strategy-new',
        taskId: 'fake-task-new',
        prompt: '新しい分析の依頼',
        phase: 'running',
        createdAt: '2099-02-02T23:00:00Z',
        notes: [
          {
            id: 'fake-note',
            title: '架空銘柄の分析ノート',
            versionId: 'fake-note-version',
          },
        ],
        notesState: 'success',
      },
      {
        strategyId: 'fake-strategy-old',
        taskId: 'fake-task-old',
        prompt: '古い分析の依頼',
        phase: 'completed',
        createdAt: '2099-02-01T00:00:00Z',
        notes: [],
        notesState: 'success',
      },
    ]

    renderAnalyses(analyses, 'success')
    const section = await screen.findByRole('region', {
      name: 'この取引について頼んだ分析',
    })
    expect(readAnalysesOutput(section)).toEqual({
      heading: '>この取引について頼んだ分析',
      rows: [
        {
          prompt: '新しい分析の依頼',
          phase: 'RUNNING',
          date: '2099-02-03',
          notes: {
            noteLinks: [
              {
                label: '架空銘柄の分析ノート',
                href: '/notes/fake-note?version_id=fake-note-version',
              },
            ],
            status: null,
            alert: null,
            hasNoNotesMessage: false,
          },
          runDetail: {
            label: '実行詳細 →',
            href: '/strategies/fake-strategy-new/runs/fake-task-new',
          },
        },
        {
          prompt: '古い分析の依頼',
          phase: 'COMPLETED',
          date: '2099-02-01',
          notes: {
            noteLinks: [],
            status: null,
            alert: null,
            hasNoNotesMessage: true,
          },
          runDetail: {
            label: '実行詳細 →',
            href: '/strategies/fake-strategy-old/runs/fake-task-old',
          },
        },
      ],
    })
  })

  it('shows an empty state when no analysis was requested for the trade', async () => {
    renderAnalyses([], 'success')
    const section = await screen.findByRole('region', {
      name: 'この取引について頼んだ分析',
    })

    expect(readSectionStateOutput(section)).toEqual({
      busy: 'false',
      text: '>この取引について頼んだ分析この取引について頼んだ分析はありません。',
      status: null,
      alert: null,
    })
  })

  it('shows a loading state while analyses are loading', async () => {
    renderAnalyses([], 'loading')
    const section = await screen.findByRole('region', {
      name: 'この取引について頼んだ分析',
    })

    expect(readSectionStateOutput(section)).toEqual({
      busy: 'true',
      text: '>この取引について頼んだ分析分析を読み込み中です。',
      status: '分析を読み込み中です。',
      alert: null,
    })
  })

  it('shows an error state when analyses cannot be loaded', async () => {
    renderAnalyses([], 'error')
    const section = await screen.findByRole('region', {
      name: 'この取引について頼んだ分析',
    })

    expect(readSectionStateOutput(section)).toEqual({
      busy: 'false',
      text: '>この取引について頼んだ分析頼んだ分析を読み込めませんでした。',
      status: null,
      alert: '頼んだ分析を読み込めませんでした。',
    })
  })

  it.each([
    {
      notesState: 'loading' as const,
      status: 'ノートを読み込み中です。',
      alert: null,
    },
    {
      notesState: 'error' as const,
      status: null,
      alert: 'ノートを読み込めませんでした。',
    },
  ])('shows the task note $notesState state', async (noteState) => {
    renderAnalyses(
      [
        {
          strategyId: 'fake-strategy',
          taskId: 'fake-task',
          prompt: '架空銘柄の分析を依頼する',
          phase: 'completed',
          createdAt: '2099-02-03T00:00:00Z',
          notes: [],
          notesState: noteState.notesState,
        },
      ],
      'success',
    )
    const section = await screen.findByRole('region', {
      name: 'この取引について頼んだ分析',
    })

    expect(readAnalysesOutput(section)).toEqual({
      heading: '>この取引について頼んだ分析',
      rows: [
        {
          prompt: '架空銘柄の分析を依頼する',
          phase: 'COMPLETED',
          date: '2099-02-03',
          notes: {
            noteLinks: [],
            status: noteState.status,
            alert: noteState.alert,
            hasNoNotesMessage: false,
          },
          runDetail: {
            label: '実行詳細 →',
            href: '/strategies/fake-strategy/runs/fake-task',
          },
        },
      ],
    })
  })
})
