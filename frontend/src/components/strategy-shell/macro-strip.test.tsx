import { cleanup, render } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { MacroStripView } from '#components/strategy-shell/macro-strip'

afterEach(cleanup)

const SAMPLE = [
  {
    symbol: '日経225',
    value: '38420.55',
    pct: -0.62,
    fetched_at: '2026-06-25T06:00:00Z',
  },
  {
    symbol: 'USD/JPY',
    value: '157.84',
    pct: 0.38,
    fetched_at: '2026-06-25T06:00:00Z',
  },
]

function snapshot(container: HTMLElement): string {
  return container.textContent.replace(/\s+/g, ' ').trim()
}

describe('MacroStripView', () => {
  it('fresh state ではティックを描画し stale バッジは出さない', () => {
    const { container } = render(
      <MacroStripView ticks={SAMPLE} staleSince={null} isPending={false} />,
    )
    expect(snapshot(container)).toBe(
      '##macro日経22538420.55-0.62%USD/JPY157.84+0.38%',
    )
  })

  it('stale state では値と stale バッジを両方出す', () => {
    const { container } = render(
      <MacroStripView
        ticks={SAMPLE}
        staleSince="2026-06-25T01:00:00Z"
        isPending={false}
      />,
    )
    expect(snapshot(container)).toBe(
      '##macrostale日経22538420.55-0.62%USD/JPY157.84+0.38%',
    )
  })

  it('ticks=null では N/A 表示する', () => {
    const { container } = render(
      <MacroStripView
        ticks={null}
        staleSince="2026-06-23T00:00:00Z"
        isPending={false}
      />,
    )
    expect(snapshot(container)).toBe('##macroN/A')
  })

  it('初回 loading 中はプレースホルダを出す', () => {
    const { container } = render(
      <MacroStripView ticks={null} staleSince={null} isPending={true} />,
    )
    expect(snapshot(container)).toBe('##macroloading…')
  })
})
