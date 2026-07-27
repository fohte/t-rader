import { render } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import { HypothesisEditor } from '#components/hypothesis-detail/hypothesis-editor'

describe('HypothesisEditor', () => {
  // 親の再レンダーで initialTitle/initialBody が同じ値のまま渡されても、
  // 内部で毎回新しいオブジェクトを作って dirty 判定していると無限レンダーに陥る
  it('親が同じ値で再レンダーしても無限レンダーにならない', () => {
    const errorSpy = vi.spyOn(console, 'error').mockImplementation(() => {})
    const { rerender } = render(
      <HypothesisEditor initialTitle="a" initialBody="b" onSave={() => {}} />,
    )
    rerender(
      <HypothesisEditor initialTitle="a" initialBody="b" onSave={() => {}} />,
    )
    const messages = errorSpy.mock.calls.map((c) => String(c[0]))
    errorSpy.mockRestore()
    expect(messages.some((m) => m.includes('Maximum update depth'))).toBe(false)
  })
})
