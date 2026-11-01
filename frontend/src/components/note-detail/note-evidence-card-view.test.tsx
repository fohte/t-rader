import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { NoteEvidenceCardView } from '#components/note-detail/note-evidence-card-view'
import type { NoteChangeEvidence } from '#lib/note-change-evidence'
import type { NoteIndicatorChangeEvidenceState } from '#lib/note-indicator-change-evidence'
import { createStoryRouter } from '#storybook/story-router'

afterEach(cleanup)

async function noteEvidenceCardOutput(rowLabel: string) {
  const links = await screen.findAllByRole('link')
  const evidenceList = screen.getByText(rowLabel).closest('dl')

  return {
    links: links.map((link) => ({
      label: link.textContent,
      href: link.getAttribute('href'),
    })),
    evidenceRows: Array.from(
      evidenceList?.querySelectorAll('dt, dd') ?? [],
      (child) => child.textContent,
    ),
  }
}

function indicatorChangeCardOutput(title: string) {
  const evidenceList = screen.getByText('期間').closest('dl')
  const status = screen.queryByRole('status') ?? screen.queryByRole('alert')
  const calculationLabel = screen.queryByText('計算 (現在の観測値)')

  return {
    title: screen.getByText(title).textContent,
    evidenceRows: Array.from(
      evidenceList?.querySelectorAll('dt, dd') ?? [],
      (child) => child.textContent,
    ),
    status: status?.textContent ?? null,
    calculation: calculationLabel?.nextElementSibling?.textContent ?? null,
    links: screen.getAllByRole('link').map((link) => ({
      label: link.textContent,
      href: link.getAttribute('href'),
    })),
  }
}

describe('NoteEvidenceCardView', () => {
  it.each([
    {
      name: 'uses the price date as the destination date',
      displayValue: '3,150',
      reference: {
        kind: 'price' as const,
        stockName: 'デモ商事',
        stockId: '0000',
        field: 'close' as const,
        date: '2030-01-03',
      },
      expectedHref: '/stocks/0000?date=2030-01-03',
      expectedLabel: '銘柄ページで 2030-01-03 を見る →',
      expectedEvidenceRows: [
        '時点',
        '2030-01-03',
        '出どころ',
        '日足 (分割調整済み)',
      ],
    },
    {
      name: 'uses the end date as the change destination date',
      displayValue: '+8.62%',
      reference: {
        kind: 'change' as const,
        stockName: 'デモ商事',
        stockId: '0000',
        field: 'close' as const,
        start: '2030-01-02',
        end: '2030-01-03',
        barsState: {
          status: 'ready' as const,
          evidence: {
            startValue: 2900,
            endValue: 3150,
            changePercent: (250 / 2900) * 100,
            matchesSavedValue: true,
          } satisfies NoteChangeEvidence,
        },
      },
      expectedHref: '/stocks/0000?date=2030-01-03',
      expectedLabel: '銘柄ページで 2030-01-03 を見る →',
      expectedEvidenceRows: [
        '期間',
        '2030-01-02 → 2030-01-03',
        '出どころ',
        '日足 (分割調整済み)',
      ],
    },
    {
      name: 'uses the valuation date as the destination date',
      displayValue: '18.4',
      reference: {
        kind: 'valuation' as const,
        stockName: 'デモ商事',
        stockId: '0000',
        field: 'per' as const,
        date: '2030-01-03',
      },
      expectedHref: '/stocks/0000?date=2030-01-03',
      expectedLabel: '銘柄ページで 2030-01-03 を見る →',
      expectedEvidenceRows: [
        '時点',
        '2030-01-03',
        '出どころ',
        'バリュエーション',
      ],
    },
    {
      name: 'shows split adjustment in the valuation source',
      displayValue: '125',
      reference: {
        kind: 'valuation' as const,
        stockName: 'デモ商事',
        stockId: '0000',
        field: 'eps' as const,
        date: '2030-01-03',
      },
      expectedHref: '/stocks/0000?date=2030-01-03',
      expectedLabel: '銘柄ページで 2030-01-03 を見る →',
      expectedEvidenceRows: [
        '時点',
        '2030-01-03',
        '出どころ',
        'バリュエーション (分割調整済み)',
      ],
    },
    {
      name: 'opens the indicator page',
      displayValue: '142.5',
      reference: {
        kind: 'indicator' as const,
        indicatorName: 'デモ指標',
        indicatorId: 'demo-indicator',
        date: '2030-01-03',
      },
      expectedHref: '/indicators/demo-indicator',
      expectedLabel: '指標ページを見る →',
      expectedEvidenceRows: ['時点', '2030-01-03', '出どころ', 'マクロ指標'],
    },
  ])(
    '$name',
    async ({
      displayValue,
      reference,
      expectedHref,
      expectedLabel,
      expectedEvidenceRows,
    }) => {
      const router = createStoryRouter(
        () => (
          <NoteEvidenceCardView
            open
            onOpenChange={() => undefined}
            displayValue={displayValue}
            reference={reference}
          />
        ),
        {
          paths: ['/stocks/$stockId', '/indicators/$indicatorId'],
          initialPath: '/stocks/0000',
        },
      )
      render(<RouterProvider router={router} />)

      const actual = await noteEvidenceCardOutput(
        reference.kind === 'change' ? '期間' : '時点',
      )
      expect(actual).toEqual({
        links: [{ label: expectedLabel, href: expectedHref }],
        evidenceRows: expectedEvidenceRows,
      })
    },
  )

  it.each([
    {
      name: 'shows both observations and the current calculation',
      indicatorName: 'デモ指標',
      title: 'デモ指標 (demo-indicator) · 騰落率',
      state: {
        status: 'ready',
        evidence: { startValue: 100, endValue: 112.5, changePercent: 12.5 },
      },
      values: ['始点の値', '100', '終点の値', '112.5'],
      status: null,
      calculation: '(112.5 − 100) ÷ 100 × 100 = +12.50%',
    },
    {
      name: 'shows loading feedback',
      indicatorName: 'デモ指標',
      title: 'デモ指標 (demo-indicator) · 騰落率',
      state: { status: 'loading' },
      values: [],
      status: '指標の観測値を読み込み中...',
      calculation: null,
    },
    {
      name: 'shows a retrieval error',
      indicatorName: 'デモ指標',
      title: 'デモ指標 (demo-indicator) · 騰落率',
      state: { status: 'error' },
      values: [],
      status: '指標の観測値を取得できませんでした',
      calculation: null,
    },
    {
      name: 'identifies a date without an observation',
      indicatorName: 'デモ指標',
      title: 'デモ指標 (demo-indicator) · 騰落率',
      state: { status: 'missing', missingDates: ['2030-01-03'] },
      values: [],
      status: '指定日の指標観測値がありません: 2030-01-03',
      calculation: null,
    },
    {
      name: 'shows both observations when the start value is zero',
      indicatorName: 'デモ指標',
      title: 'デモ指標 (demo-indicator) · 騰落率',
      state: { status: 'zero-start', startValue: 0, endValue: 112.5 },
      values: ['始点の値', '0', '終点の値', '112.5'],
      status: '始点が 0 のため、指標の騰落率を計算できません',
      calculation: null,
    },
    {
      name: 'falls back to the ID when the indicator name is unavailable',
      indicatorName: null,
      title: 'demo-indicator · 騰落率',
      state: {
        status: 'ready',
        evidence: { startValue: 100, endValue: 112.5, changePercent: 12.5 },
      },
      values: ['始点の値', '100', '終点の値', '112.5'],
      status: null,
      calculation: '(112.5 − 100) ÷ 100 × 100 = +12.50%',
    },
  ] satisfies Array<{
    name: string
    indicatorName: string | null
    title: string
    state: NoteIndicatorChangeEvidenceState
    values: string[]
    status: string | null
    calculation: string | null
  }>)(
    '$name',
    async ({ state, indicatorName, title, values, status, calculation }) => {
      const router = createStoryRouter(
        () => (
          <NoteEvidenceCardView
            open
            onOpenChange={() => undefined}
            displayValue="+12.5%"
            reference={{
              kind: 'indicator-change',
              indicatorName,
              indicatorId: 'demo-indicator',
              start: '2030-01-02',
              end: '2030-01-03',
              state,
            }}
          />
        ),
        {
          paths: ['/stocks/$stockId', '/indicators/$indicatorId'],
          initialPath: '/indicators/demo-indicator',
        },
      )
      render(<RouterProvider router={router} />)
      await screen.findByRole('link', { name: '指標ページを見る' })

      expect(indicatorChangeCardOutput(title)).toEqual({
        title,
        evidenceRows: [
          '期間',
          '2030-01-02 → 2030-01-03',
          ...values,
          '出どころ',
          '現在の観測値',
        ],
        status,
        calculation,
        links: [
          {
            label: '指標ページを見る →',
            href: '/indicators/demo-indicator',
          },
        ],
      })
    },
  )
})
