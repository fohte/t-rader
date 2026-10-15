import { RouterProvider } from '@tanstack/react-router'
import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { NoteEvidenceCardView } from '#components/note-detail/note-evidence-card-view'
import type { NoteChangeEvidence } from '#lib/note-change-evidence'
import { createStoryRouter } from '#storybook/story-router'

afterEach(cleanup)

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
    },
  ])(
    '$name',
    async ({ displayValue, reference, expectedHref, expectedLabel }) => {
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

      const links = await screen.findAllByRole('link')
      expect(
        links.map((link) => ({
          label: link.textContent,
          href: link.getAttribute('href'),
        })),
      ).toEqual([{ label: expectedLabel, href: expectedHref }])
    },
  )
})
