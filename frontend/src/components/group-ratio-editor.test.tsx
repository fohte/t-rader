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

    await user.selectOptions(screen.getByLabelText('分類軸'), 'example-axis-a')

    expect(onRowsChange).toHaveBeenLastCalledWith([
      { axis: 'example-axis-a', ratioPercent: '25' },
    ])
  })
})
