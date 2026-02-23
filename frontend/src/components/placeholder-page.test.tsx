import { cleanup, render, screen } from '@testing-library/react'
import { History } from 'lucide-react'
import { afterEach, describe, expect, it } from 'vitest'

import { PlaceholderPage } from '@/components/placeholder-page'

afterEach(cleanup)

describe('PlaceholderPage', () => {
  it('タイトル、説明、開発中メッセージを表示する', () => {
    render(
      <PlaceholderPage
        title="トレード履歴"
        description="売買記録の一覧と振り返りができます"
        icon={History}
      />,
    )

    expect(screen.getByText('トレード履歴')).toBeInTheDocument()
    expect(
      screen.getByText('売買記録の一覧と振り返りができます'),
    ).toBeInTheDocument()
    expect(screen.getByText('この機能は開発中です')).toBeInTheDocument()
  })

  it('渡されたアイコンをレンダリングする', () => {
    const { container } = render(
      <PlaceholderPage
        title="テスト"
        description="テスト説明"
        icon={History}
      />,
    )

    // lucide-react はアイコンを svg 要素としてレンダリングする
    const svg = container.querySelector('svg')
    expect(svg).toBeInTheDocument()
  })
})
