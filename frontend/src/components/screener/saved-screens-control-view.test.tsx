import { cleanup, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it } from 'vitest'

import { SavedScreensControlView } from '#components/screener/saved-screens-control-view'
import type { SavedScreen } from '#lib/screener-search'

const savedScreen: SavedScreen = {
  id: 'saved-screen-a',
  name: '試作条件',
  created_at: '2099-01-01T00:00:00Z',
  updated_at: '2099-01-02T00:00:00Z',
  filters: {
    markets: [],
    sectors: [],
    per: null,
    pbr: null,
    roe: null,
    dividend_yield: null,
    market_cap: null,
  },
  sort: { field: 'name', direction: 'asc' },
}

const baseArgs = {
  savedScreens: [savedScreen],
  selectedScreen: savedScreen,
  selectedScreenId: savedScreen.id,
  menuOpen: false,
  dialog: null,
  onMenuOpenChange: () => {},
  onDialogChange: () => {},
  onSelect: () => {},
  onSave: () => {},
  onOverwrite: () => {},
  onDelete: () => {},
} satisfies React.ComponentProps<typeof SavedScreensControlView>

afterEach(cleanup)

describe('SavedScreensControlView', () => {
  it('shows the selected saved-screen name with its purpose', () => {
    render(<SavedScreensControlView {...baseArgs} />)

    const trigger = screen.getByRole('button', {
      name: '保存した条件: 試作条件',
    })

    expect(trigger.textContent).toBe('保存した条件: 試作条件')
  })

  it('keeps the delete confirmation visible when the delete dialog is selected', () => {
    render(
      <SavedScreensControlView
        {...baseArgs}
        dialog={{ type: 'delete', savedScreen }}
      />,
    )

    const dialog = screen.getByRole('dialog')

    expect(dialog.textContent).toBe(
      '保存した条件を削除「試作条件」を削除します。この操作は取り消せません。キャンセル削除Close',
    )
  })
})
