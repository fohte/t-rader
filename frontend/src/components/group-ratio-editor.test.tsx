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

    expect(onRowsChange).toHaveBeenLastCalledWith([
      { axis: '', ratioPercent: '' },
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

    expect(onRowsChange).toHaveBeenLastCalledWith([
      { axis: 'example-axis-a', ratioPercent: '25' },
    ])
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
