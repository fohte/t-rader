import type { Meta, StoryObj } from '@storybook/react-vite'

import { SavedScreensControlView } from '#components/screener/saved-screens-control-view'
import type { SavedScreen } from '#lib/screener-search'

const savedScreen: SavedScreen = {
  id: 'saved-screen-a',
  name: '試作条件',
  created_at: '2099-01-01T00:00:00Z',
  updated_at: '2099-01-02T00:00:00Z',
  filters: {
    markets: ['プライム'],
    sectors: [],
    per: { min: null, max: 12 },
    pbr: null,
    roe: null,
    dividend_yield: null,
    market_cap: null,
  },
  sort: { field: 'market_cap', direction: 'desc' },
}

const args = {
  savedScreens: [savedScreen],
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

const meta = {
  title: 'Screener/SavedScreensControlView',
  component: SavedScreensControlView,
  decorators: [(Story) => <div className="min-h-40 p-6">{Story()}</div>],
} satisfies Meta<typeof SavedScreensControlView>

export default meta
type Story = StoryObj<typeof meta>

export const Closed: Story = {
  name: '保存した条件を選ぶメニューを閉じています。',
  args: { ...args, selectedScreenId: undefined, savedScreens: [] },
}

export const MenuOpen: Story = {
  name: '保存した条件と編集操作を一覧で表示します。',
  args: { ...args, menuOpen: true },
}

export const ModifiedMenuOpen: Story = {
  name: '変更した条件を上書き保存できるメニューを表示します。',
  args: { ...args, menuOpen: true, selectedScreenHasChanges: true },
}

export const Loading: Story = {
  name: '保存した条件を読み込んでいます。',
  args: {
    ...args,
    savedScreens: [],
    selectedScreenId: savedScreen.id,
    menuOpen: true,
    listPending: true,
  },
}

export const ListError: Story = {
  name: '保存した条件を読み込めない状態を表示します。',
  args: { ...args, menuOpen: true, listError: true },
}

export const EmptyMenu: Story = {
  name: '保存した条件がないことをメニュー内に表示します。',
  args: {
    ...args,
    selectedScreenId: undefined,
    savedScreens: [],
    menuOpen: true,
  },
}

export const SaveDialog: Story = {
  name: '現在の条件に名前を付ける dialog を表示します。',
  args: { ...args, dialog: { type: 'save' } },
}

export const SaveError: Story = {
  name: '同じ名前の条件があるときに保存エラーを表示します。',
  args: {
    ...args,
    dialog: { type: 'save' },
    error: '同じ名前の保存条件がすでにあります。',
  },
}

export const OverwriteError: Story = {
  name: '上書きに失敗した理由を条件パネルに表示します。',
  args: {
    ...args,
    error: '下限は上限以下にしてください。',
  },
}

export const RenameDialog: Story = {
  name: '選択した保存条件の名前を変更する dialog を表示します。',
  args: { ...args, dialog: { type: 'rename', savedScreen } },
}

export const DeleteConfirmation: Story = {
  name: '保存した条件を削除する前の確認を表示します。',
  args: { ...args, dialog: { type: 'delete', savedScreen } },
}

export const DeleteError: Story = {
  name: '保存した条件の削除に失敗した理由を表示します。',
  args: {
    ...args,
    dialog: { type: 'delete', savedScreen },
    error: '保存した条件を削除できませんでした。',
  },
}
