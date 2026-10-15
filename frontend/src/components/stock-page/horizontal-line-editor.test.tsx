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
        targetKinds={['sample-kind']}
        onSave={onSave}
        onCancel={() => undefined}
      />,
    )

    await user.type(
      screen.getByRole('combobox', { name: '種別キー' }),
      'sample-kind',
    )
    await user.type(
      screen.getByRole('textbox', { name: 'メモ' }),
      '支持線として確認する',
    )
    await user.click(screen.getByRole('button', { name: '保存' }))

    expect(onSave.mock.calls).toEqual([
      [
        {
          target_kind: 'sample-kind',
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

    await user.click(screen.getByRole('button', { name: '取消' }))

    expect(onCancel.mock.calls).toEqual([[]])
  })

  it('does not save whitespace-only kind and note values', async () => {
    const onSave = vi.fn()
    const user = userEvent.setup()

    render(
      <HorizontalLineEditor
        timestamp="2030-04-02T06:00:00Z"
        price={3110}
        targetKinds={[]}
        onSave={onSave}
        onCancel={() => undefined}
      />,
    )

    await user.type(screen.getByRole('combobox', { name: '種別キー' }), '   ')
    await user.type(screen.getByRole('textbox', { name: 'メモ' }), '   ')
    await user.click(screen.getByRole('button', { name: '保存' }))

    function readFormResult() {
      return {
        saveCalls: onSave.mock.calls,
        error: screen.getByRole('alert').textContent,
      }
    }

    expect(readFormResult()).toEqual({
      saveCalls: [],
      error: '種別キーとメモを入力してください',
    })
  })
})
