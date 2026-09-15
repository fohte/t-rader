import { render, screen } from '@testing-library/react'
import { describe, expect, it, vi } from 'vitest'

import { CodeEditor } from '#components/indicators/code-editor'

vi.mock(
  '@monaco-editor/react',
  () => import('#components/indicators/__mocks__/monaco-editor-react'),
)
// monaco-setup は実物の monaco-editor を import するため jsdom では評価できない
vi.mock('#components/indicators/monaco-setup', () => ({}))

describe('CodeEditor', () => {
  it('readOnly を渡すと編集不可になる', () => {
    render(
      <CodeEditor
        language="python"
        value="x = 1"
        onChange={vi.fn()}
        ariaLabel="code"
        readOnly
      />,
    )

    expect(screen.getByLabelText('code')).toHaveAttribute('readonly')
  })
})
