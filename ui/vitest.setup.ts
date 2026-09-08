import '@testing-library/jest-dom/vitest';

/**
 * jsdom implements neither of these, and the Skeleton components that wrap
 * zag-js reach for both — the segmented control's sliding indicator measures
 * its items to position itself.
 *
 * Stubbed rather than shimmed with a real implementation: these tests assert
 * behaviour and semantics, not geometry, and a stub that silently reports no
 * observations is honest about that. Anything that genuinely depends on
 * measured layout belongs in the Playwright suite, which runs in a real
 * browser.
 */
class NoopObserver {
  observe(): void {}
  unobserve(): void {}
  disconnect(): void {}
  takeRecords(): [] {
    return [];
  }
}

globalThis.ResizeObserver ??= NoopObserver as unknown as typeof ResizeObserver;
globalThis.IntersectionObserver ??=
  NoopObserver as unknown as typeof IntersectionObserver;

if (!Element.prototype.scrollIntoView) {
  Element.prototype.scrollIntoView = () => {};
}
