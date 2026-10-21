import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { GroupRatioEditor } from '#components/group-ratio-editor'

const axes = [
  { key: 'example-axis-a', name: 'Example dimension A' },
  { key: 'example-axis-b', name: 'Example dimension B' },
]

afterEach(() => {
  cleanup()
})

function readAxisSelectionResult(
  unavailableAxisLabel: string | null,
  changes: unknown,
) {
  return { unavailableAxisLabel, changes }
}

describe('GroupRatioEditor', () => {
  it('appending a limit emits an empty draft row', async () => {
    const user = userEvent.setup()
    const onRowsChange = vi.fn()
    render(
      <GroupRatioEditor
        axes={axes}
        rows={[]}
        errors={[]}
        isLoading={false}
        loadError={false}
        isSaving={false}
        saveError={null}
        onRowsChange={onRowsChange}
        onSave={vi.fn()}
      />,
    )

    await user.click(screen.getByRole('button', { name: '分類軸を追加' }))

    expect(onRowsChange.mock.calls).toEqual([
      [[{ axis: '', ratioPercent: '' }]],
    ])
  })

  it('changing a selected axis emits the complete updated draft', async () => {
    const user = userEvent.setup()
    const onRowsChange = vi.fn()
    render(
      <GroupRatioEditor
        axes={axes}
        rows={[{ axis: '', ratioPercent: '25' }]}
        errors={[]}
        isLoading={false}
        loadError={false}
        isSaving={false}
        saveError={null}
        onRowsChange={onRowsChange}
        onSave={vi.fn()}
      />,
    )

    await user.click(screen.getByRole('combobox', { name: '分類軸' }))
    await user.click(
      await screen.findByRole('option', { name: 'Example dimension A' }),
    )

    expect(onRowsChange.mock.calls).toEqual([
      [[{ axis: 'example-axis-a', ratioPercent: '25' }]],
    ])
  }, 10_000)

  it('preserves an unavailable axis until a current axis is selected', async () => {
    const user = userEvent.setup()
    const onRowsChange = vi.fn()
    render(
      <GroupRatioEditor
        axes={axes}
        rows={[{ axis: 'example-axis-archived', ratioPercent: '25' }]}
        errors={[]}
        isLoading={false}
        loadError={false}
        isSaving={false}
        saveError={null}
        onRowsChange={onRowsChange}
        onSave={vi.fn()}
      />,
    )

    const unavailableAxisLabel = screen.getByRole('combobox', {
      name: '分類軸',
    }).textContent
    await user.click(screen.getByRole('combobox', { name: '分類軸' }))
    await user.click(
      await screen.findByRole('option', { name: 'Example dimension A' }),
    )

    expect(
      readAxisSelectionResult(unavailableAxisLabel, onRowsChange.mock.calls),
    ).toEqual({
      unavailableAxisLabel: 'example-axis-archived (現在利用できません)',
      changes: [[[{ axis: 'example-axis-a', ratioPercent: '25' }]]],
    })
  }, 10_000)

  it('disables classification axes already assigned to another row', async () => {
    const user = userEvent.setup()
    render(
      <GroupRatioEditor
        axes={axes}
        rows={[
          { axis: 'example-axis-a', ratioPercent: '25' },
          { axis: '', ratioPercent: '40' },
        ]}
        errors={[]}
        isLoading={false}
        loadError={false}
        isSaving={false}
        saveError={null}
        onRowsChange={vi.fn()}
        onSave={vi.fn()}
      />,
    )

    const secondSelect = screen
      .getAllByRole('combobox', { name: '分類軸' })
      .at(1)
    if (secondSelect == null) throw new Error('2 つ目の分類軸がありません')
    await user.click(secondSelect)
    const usedAxis = await screen.findByRole('option', {
      name: 'Example dimension A',
    })

    expect(usedAxis.getAttribute('aria-disabled')).toEqual('true')
  }, 10_000)
})
