import '@testing-library/jest-dom/vitest'

// jsdom には ResizeObserver が無いが React Flow (GraphRenderer 内部) がコンテナ計測に使う
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
globalThis.ResizeObserver = ResizeObserverStub

// jsdom には Pointer Events の capture 系 API が無いが、Radix UI (Select 等) が
// クリック操作の内部で呼ぶため、テストでの操作をエラーにしないためのスタブが要る
Element.prototype.hasPointerCapture = () => false
Element.prototype.setPointerCapture = () => {}
Element.prototype.releasePointerCapture = () => {}
Element.prototype.scrollIntoView = () => {}

// jsdom の Request コンストラクタは相対 URL を受け付けない。
// 本番 (ブラウザ) では `/api/...` 相対パスをそのまま `new Request()` に渡しても
// document.baseURI 基準で解決されるが、テストでは事前に絶対 URL 化する必要がある。
const OriginalRequest = globalThis.Request
class PatchedRequest extends OriginalRequest {
  constructor(input: RequestInfo | URL, init?: RequestInit) {
    if (typeof input === 'string' && input.startsWith('/')) {
      super(new URL(input, window.location.origin).toString(), init)
    } else {
      super(input, init)
    }
  }
}
globalThis.Request = PatchedRequest
