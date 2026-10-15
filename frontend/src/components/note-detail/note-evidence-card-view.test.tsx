import { RouterProvider } from '@tanstack/react-router'
import { render, screen } from '@testing-library/react'
import { describe, expect, it } from 'vitest'

import { NoteEvidenceCardView } from '#components/note-detail/note-evidence-card-view'
import type { NoteChangeEvidence } from '#lib/note-change-evidence'
import { createStoryRouter } from '#storybook/story-router'

describe('NoteEvidenceCardView', () => {
  it.each([
    {
      name: 'uses the price date as the destination date',
      displayValue: '3,150',
      evidence: { kind: 'price' as const, date: '2030-01-03' },
      expectedHref: '/stocks/0000?date=2030-01-03',
      expectedLabel: '銘柄ページで 2030-01-03 を見る →',
    },
    {
      name: 'uses the end date as the change destination date',
      displayValue: '+8.62%',
      evidence: {
        kind: 'change' as const,
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
  ])(
    '$name',
    async ({ displayValue, evidence, expectedHref, expectedLabel }) => {
      const router = createStoryRouter(
        () => (
          <NoteEvidenceCardView
            open
            onOpenChange={() => undefined}
            stockName="デモ商事"
            stockId="0000"
            field="close"
            displayValue={displayValue}
            evidence={evidence}
          />
        ),
        { paths: ['/stocks/$stockId'], initialPath: '/stocks/0000' },
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
