import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import { afterEach, describe, expect, it, vi } from 'vitest'

import { AnnotationDraftForm } from '#components/stock-page/annotation-draft-form'

afterEach(cleanup)

describe('AnnotationDraftForm', () => {
  it('saves trimmed freeform annotation fields', () => {
    const onSave = vi.fn()
    render(
      <AnnotationDraftForm
        ariaLabel="アノテーションの入力"
        header={<h2>新しいメモ</h2>}
        targetKinds={['fake-kind']}
        onSave={onSave}
        onCancel={() => {}}
      />,
    )

    fireEvent.change(screen.getByRole('combobox', { name: '種別キー' }), {
      target: { value: '  custom-kind  ' },
    })
    fireEvent.change(screen.getByRole('textbox', { name: 'メモ' }), {
      target: { value: '  fake memo  ' },
    })
    fireEvent.click(screen.getByRole('button', { name: '保存' }))

    expect(onSave.mock.calls).toEqual([
      [{ target_kind: 'custom-kind', text: 'fake memo' }],
    ])
  })
})
