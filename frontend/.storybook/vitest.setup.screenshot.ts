import { screenshot } from '@storycap-testrun/browser'
import { afterEach, beforeEach, vi } from 'vitest'
import { page } from 'vitest/browser'

import { SCREENSHOT_VIEWPORT } from './screenshot-viewport'

// story の props に固定タイムスタンプを渡していても formatRelative 等の相対時刻表示は
// 実行時刻基準で変わるため、システム時刻を固定してスクリーンショットを決定的にする
vi.setSystemTime(new Date('2026-01-15T10:00:00+09:00'))

// スクロールバー (ネイティブ/Monaco Editor 独自の両方) はホバー状態やフェード
// タイマーでサム/トラックの見え方が撮影ごとに 1px 未満揺れる。撮影時は非表示にして
// スクロール可否の判定に影響しない見た目の揺れを潰す
const style = document.createElement('style')
style.textContent = `
  ::-webkit-scrollbar { display: none !important; }
  .monaco-scrollable-element > .scrollbar { visibility: hidden !important; }
`
document.head.appendChild(style)

beforeEach(async () => {
  await page.viewport(SCREENSHOT_VIEWPORT.width, SCREENSHOT_VIEWPORT.height)
})

// Playwright のキャレット非表示等の撮影前処理は保留中の再描画を待たないため、
// rAF を 2 段ネストして 1 フレーム分の描画が実際に反映されるのを待ってから撮影する
async function waitForPaint(): Promise<void> {
  await new Promise<void>((resolve) => {
    requestAnimationFrame(() => {
      requestAnimationFrame(() => {
        resolve()
      })
    })
  })
}

afterEach(async (context) => {
  await waitForPaint()
  await screenshot(page, context)
})
