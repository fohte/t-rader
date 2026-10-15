import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { HorizontalLineEditor } from '#components/stock-page/horizontal-line-editor'

afterEach(cleanup)

describe('HorizontalLineEditor', () => {
  it('saves a horizontal line with its kind, note, timestamp, and price', async () => {
    const onSave = vi.fn()
    const user = userEvent.setup()

    render(
      <HorizontalLineEditor
        timestamp="2030-04-02T06:00:00Z"
        price={3110}
        targetKinds={['sample-level']}
        onSave={onSave}
        onCancel={() => undefined}
      />,
    )

    await user.type(
      screen.getByRole('combobox', { name: '種別キー' }),
      'sample-level',
    )
    await user.type(
      screen.getByRole('textbox', { name: 'メモ' }),
      '支持線として確認する',
    )
    await user.click(screen.getByRole('button', { name: '保存' }))

    expect(onSave.mock.calls).toEqual([
      [
        {
          target_kind: 'sample-level',
          text: '支持線として確認する',
          timestamp: '2030-04-02T06:00:00Z',
          price: 3110,
        },
      ],
    ])
  })

  it('cancels the uncommitted line', async () => {
    const onCancel = vi.fn()
    const user = userEvent.setup()

    render(
      <HorizontalLineEditor
        timestamp="2030-04-02T06:00:00Z"
        price={3110}
        targetKinds={[]}
        onSave={() => undefined}
        onCancel={onCancel}
      />,
    )

    await user.click(screen.getByRole('button', { name: '取消 (Esc)' }))

    expect(onCancel.mock.calls).toEqual([[]])
  })
})
