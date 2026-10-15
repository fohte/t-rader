import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { NoteEvidenceCardView } from '#components/note-detail/note-evidence-card-view'
import type { NoteChangeEvidence } from '#lib/note-change-evidence'
import { createStoryRouter } from '#storybook/story-router'

afterEach(cleanup)

async function noteEvidenceCardOutput(rowLabel: string) {
  const links = await screen.findAllByRole('link')
  const evidenceRows = screen.getByText(rowLabel).parentElement

  return {
    links: links.map((link) => ({
      label: link.textContent,
      href: link.getAttribute('href'),
    })),
    evidenceRows:
      evidenceRows == null
        ? []
        : Array.from(evidenceRows.children, (child) => child.textContent),
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
})
