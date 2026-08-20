import { screenshot } from '@storycap-testrun/browser'
import { afterEach, beforeEach, vi } from 'vitest'
import { page } from 'vitest/browser'

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

// ponytail: 上記でも Monaco Editor を含む一部の story は撮影のたびに数十ピクセル
// 未満 (0.006% 程度、内容ではなく canvas 描画のサブピクセル差) だけ揺れることがある。
// Chromium 起動オプション (--disable-gpu 等) や撮影前ディレイでも解消しなかった。
// reg-suit 側で許容する閾値を設ける方針が妥当。閾値運用は別タスク (reg-suit 導入) の責務
beforeEach(async () => {
  await page.viewport(1280, 800)
})

afterEach(async (context) => {
  await screenshot(page, context)
})
