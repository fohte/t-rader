import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { TrendLineEditor } from '#components/stock-page/trend-line-editor'

afterEach(cleanup)

describe('TrendLineEditor', () => {
  it('saves both endpoints with the annotation kind and note', async () => {
    const onSave = vi.fn()
    const user = userEvent.setup()

    render(
      <TrendLineEditor
        start={{ timestamp: '2030-04-02T06:00:00Z', price: 3120 }}
        end={{ timestamp: '2030-04-01T06:00:00Z', price: 3390 }}
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
      '架空の傾きを記録する',
    )
    await user.click(screen.getByRole('button', { name: '保存' }))

    expect(onSave.mock.calls).toEqual([
      [
        {
          start: { timestamp: '2030-04-02T06:00:00Z', price: 3120 },
          end: { timestamp: '2030-04-01T06:00:00Z', price: 3390 },
          target_kind: 'sample-kind',
          text: '架空の傾きを記録する',
        },
      ],
    ])
  })

  it('does not save whitespace-only kind and note values', async () => {
    const onSave = vi.fn()
    const user = userEvent.setup()

    render(
      <TrendLineEditor
        start={{ timestamp: '2030-04-01T06:00:00Z', price: 3390 }}
        end={{ timestamp: '2030-04-02T06:00:00Z', price: 3120 }}
        targetKinds={[]}
        onSave={onSave}
        onCancel={() => undefined}
      />,
    )

    await user.type(screen.getByRole('combobox', { name: '種別キー' }), '   ')
    await user.type(screen.getByRole('textbox', { name: 'メモ' }), '   ')
    await user.click(screen.getByRole('button', { name: '保存' }))

    function readSaveState() {
      return {
        saveCalls: onSave.mock.calls,
        error: screen.getByRole('alert').textContent,
      }
    }

    expect(readSaveState()).toEqual({
      saveCalls: [],
      error: '種別キーとメモを入力してください',
    })
  })
})
