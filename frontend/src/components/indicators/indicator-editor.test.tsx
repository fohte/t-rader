import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, describe, expect, it, vi } from 'vitest'

import {
  IndicatorEditor,
  type IndicatorEditorValue,
  type PreviewState,
} from '#components/indicators/indicator-editor'

vi.mock(
  '@monaco-editor/react',
  () => import('#components/indicators/__mocks__/monaco-editor-react'),
)

afterEach(cleanup)

const BASE_INITIAL: IndicatorEditorValue = {
  name: 'rsi',
  code: "print('{}')",
  inputSchema: '{}',
  outputSchema: '{}',
  description: '',
}

const QUIET_PREVIEW: PreviewState = {
  isRunning: false,
  error: null,
  result: null,
}

describe('IndicatorEditor', () => {
  it('initial と一致する間は保存ボタンが disabled で dirty 表示も出ない', () => {
    render(
      <IndicatorEditor
        scope="global"
        initial={BASE_INITIAL}
        onSave={() => {}}
        onPreview={() => {}}
        preview={QUIET_PREVIEW}
      />,
    )
    expect(screen.getByRole('button', { name: '保存' })).toBeDisabled()
    expect(screen.queryByTestId('dirty-indicator')).toBeNull()
  })

  it('code を編集すると dirty になり、保存ボタンに現在値が渡る', async () => {
    const user = userEvent.setup()
    const onSave = vi.fn()
    render(
      <IndicatorEditor
        scope="global"
        initial={BASE_INITIAL}
        onSave={onSave}
        onPreview={() => {}}
        preview={QUIET_PREVIEW}
      />,
    )
    const codeArea = screen.getByLabelText('indicator code')
    await user.clear(codeArea)
    await user.type(codeArea, "print('hello')")
    expect(screen.getByTestId('dirty-indicator')).toBeInTheDocument()

    await user.click(screen.getByRole('button', { name: '保存' }))
    expect(onSave).toHaveBeenCalledWith({
      ...BASE_INITIAL,
      code: "print('hello')",
    })
  })

  it('プレビュー実行ボタンを押すと code / schema / args が渡る', async () => {
    const user = userEvent.setup()
    const onPreview = vi.fn()
    render(
      <IndicatorEditor
        scope="strategy"
        initial={BASE_INITIAL}
        onSave={() => {}}
        onPreview={onPreview}
        preview={QUIET_PREVIEW}
      />,
    )

    const argsArea = screen.getByLabelText('preview args')
    fireEvent.change(argsArea, { target: { value: '{"period": 14}' } })

    await user.click(screen.getByRole('button', { name: 'プレビュー実行' }))
    expect(onPreview).toHaveBeenCalledWith({
      code: BASE_INITIAL.code,
      inputSchema: BASE_INITIAL.inputSchema,
      outputSchema: BASE_INITIAL.outputSchema,
      argsJson: '{"period": 14}',
    })
  })

  it('dirty 状態で発火した beforeunload を preventDefault する', async () => {
    const user = userEvent.setup()
    render(
      <IndicatorEditor
        scope="global"
        initial={BASE_INITIAL}
        onSave={() => {}}
        onPreview={() => {}}
        preview={QUIET_PREVIEW}
      />,
    )
    await user.type(screen.getByLabelText('indicator code'), 'X')
    const ev = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(ev)
    expect(ev.defaultPrevented).toBe(true)
  })

  it('クリーン状態の beforeunload は preventDefault しない', () => {
    render(
      <IndicatorEditor
        scope="global"
        initial={BASE_INITIAL}
        onSave={() => {}}
        onPreview={() => {}}
        preview={QUIET_PREVIEW}
      />,
    )
    const ev = new Event('beforeunload', { cancelable: true })
    window.dispatchEvent(ev)
    expect(ev.defaultPrevented).toBe(false)
  })

  it('scope ラベルに global / strategy を表示する', () => {
    const { rerender } = render(
      <IndicatorEditor
        scope="global"
        initial={BASE_INITIAL}
        onSave={() => {}}
        onPreview={() => {}}
        preview={QUIET_PREVIEW}
      />,
    )
    expect(screen.getByTestId('indicator-scope').textContent).toBe(
      'scope: global',
    )
    rerender(
      <IndicatorEditor
        scope="strategy"
        initial={BASE_INITIAL}
        onSave={() => {}}
        onPreview={() => {}}
        preview={QUIET_PREVIEW}
      />,
    )
    expect(screen.getByTestId('indicator-scope').textContent).toBe(
      'scope: strategy',
    )
  })

  it('nameReadOnly のときは name 入力が readOnly になる', () => {
    render(
      <IndicatorEditor
        scope="global"
        initial={BASE_INITIAL}
        nameReadOnly
        onSave={() => {}}
        onPreview={() => {}}
        preview={QUIET_PREVIEW}
      />,
    )
    expect(screen.getByLabelText('name')).toHaveAttribute('readonly')
  })

  it('プレビュー結果を output / stdout / stderr / exit_code に分けて表示する', () => {
    render(
      <IndicatorEditor
        scope="global"
        initial={BASE_INITIAL}
        onSave={() => {}}
        onPreview={() => {}}
        preview={{
          isRunning: false,
          error: null,
          result: {
            output: { value: 42 },
            stdout: '{"value": 42}',
            stderr: 'note',
            exit_code: 0,
          },
        }}
      />,
    )
    const result = screen.getByTestId('preview-result')
    expect(
      result.querySelector('[data-testid="preview-exit-code"]')?.textContent,
    ).toBe('0')
    expect(
      result.querySelector('[data-testid="preview-output"]')?.textContent,
    ).toBe('{\n  "value": 42\n}')
    expect(
      result.querySelector('[data-testid="preview-stdout"]')?.textContent,
    ).toBe('{"value": 42}')
    expect(
      result.querySelector('[data-testid="preview-stderr"]')?.textContent,
    ).toBe('note')
  })

  it('プレビューエラーを表示する', () => {
    render(
      <IndicatorEditor
        scope="global"
        initial={BASE_INITIAL}
        onSave={() => {}}
        onPreview={() => {}}
        preview={{
          isRunning: false,
          error: 'args が JSON として不正です',
          result: null,
        }}
      />,
    )
    expect(screen.getByTestId('preview-error').textContent).toBe(
      'args が JSON として不正です',
    )
  })
})
