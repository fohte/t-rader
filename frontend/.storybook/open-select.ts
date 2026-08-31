import { userEvent, within } from 'storybook/test'

// Base UI の Select はトリガーのみを描画し、選択肢は Portal で document.body 直下に
// 描画されるため、開いた状態を撮影するにはトリガーを canvasElement 側、listbox を
// canvasElement.ownerDocument.body 側でそれぞれスコープする必要がある
export async function openSelect(canvasElement: HTMLElement) {
  await userEvent.click(within(canvasElement).getByRole('combobox'))
  await within(canvasElement.ownerDocument.body).findByRole('listbox')
}
