import type { Meta, StoryObj } from '@storybook/react-vite'
import { useState } from 'react'

import { CodeEditor } from '#components/indicators/code-editor'

const meta = {
  title: 'Indicators/CodeEditor',
  component: CodeEditor,
} satisfies Meta<typeof CodeEditor>

export default meta
type Story = StoryObj<typeof meta>

const SAMPLE_PYTHON = `import json, sys

args = json.load(sys.stdin)["args"]
period = args.get("period", 14)
print(json.dumps({"value": period * 2}))
`

const SAMPLE_JSON = JSON.stringify(
  {
    type: 'object',
    properties: { period: { type: 'integer' } },
    required: ['period'],
  },
  null,
  2,
)

const SAMPLE_YAML = `phases:
  - key: plan
    label: 調査計画
    model: claude-opus-4
    prompt: |
      与えられた問いに対し、検証すべき仮説を立てよ。
`

function Interactive({
  language,
  initial,
}: {
  language: 'python' | 'json' | 'yaml'
  initial: string
}) {
  const [value, setValue] = useState(initial)
  return (
    <CodeEditor
      language={language}
      value={value}
      onChange={setValue}
      ariaLabel="story editor"
    />
  )
}

export const Python: Story = {
  args: { language: 'python', value: SAMPLE_PYTHON, onChange: () => {} },
  render: () => <Interactive language="python" initial={SAMPLE_PYTHON} />,
}

export const Json: Story = {
  args: { language: 'json', value: SAMPLE_JSON, onChange: () => {} },
  render: () => <Interactive language="json" initial={SAMPLE_JSON} />,
}

export const Yaml: Story = {
  args: { language: 'yaml', value: SAMPLE_YAML, onChange: () => {} },
  render: () => <Interactive language="yaml" initial={SAMPLE_YAML} />,
}

export const ReadOnly: Story = {
  args: {
    language: 'python',
    value: SAMPLE_PYTHON,
    onChange: () => {},
    readOnly: true,
  },
}
