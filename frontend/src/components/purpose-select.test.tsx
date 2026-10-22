import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { PurposeSelect } from '#components/purpose-select'

afterEach(cleanup)

describe('PurposeSelect', () => {
  it('候補一覧にない選択値も表示する', () => {
    render(
      <PurposeSelect
        purposes={['purpose-beta']}
        selectedPurpose="purpose-alpha"
        onPurposeChange={vi.fn()}
        disabled={false}
      />,
    )

    expect(
      screen.getByRole('combobox', { name: '実行目的' }).textContent,
    ).toEqual('purpose-alpha')
  })

  it('選択した設定値を callback に渡す', async () => {
    const user = userEvent.setup()
    const onPurposeChange = vi.fn()
    render(
      <PurposeSelect
        purposes={['purpose-alpha']}
        selectedPurpose=""
        onPurposeChange={onPurposeChange}
        disabled={false}
      />,
    )

    await user.click(screen.getByRole('combobox', { name: '実行目的' }))
    await user.click(
      await screen.findByRole('option', { name: 'purpose-alpha' }),
    )

    expect(onPurposeChange.mock.calls).toEqual([['purpose-alpha']])
  })

  it('既定の設定を空文字として callback に渡す', async () => {
    const user = userEvent.setup()
    const onPurposeChange = vi.fn()
    render(
      <PurposeSelect
        purposes={['purpose-alpha']}
        selectedPurpose="purpose-alpha"
        onPurposeChange={onPurposeChange}
        disabled={false}
      />,
    )

    await user.click(screen.getByRole('combobox', { name: '実行目的' }))
    await user.click(await screen.findByRole('option', { name: '既定の設定' }))

    expect(onPurposeChange.mock.calls).toEqual([['']])
  })
})
