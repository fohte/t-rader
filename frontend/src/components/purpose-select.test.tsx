import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { PurposeSelect } from '#components/purpose-select'

afterEach(cleanup)

describe('PurposeSelect', () => {
  it('選択した目的を callback に渡す', async () => {
    const onPurposeChange = vi.fn()
    render(
      <PurposeSelect
        purposes={['purpose-alpha']}
        selectedPurpose=""
        onPurposeChange={onPurposeChange}
        disabled={false}
      />,
    )

    await userEvent.selectOptions(
      screen.getByLabelText('実行目的'),
      'purpose-alpha',
    )

    expect(onPurposeChange.mock.calls).toEqual([['purpose-alpha']])
  })
})
