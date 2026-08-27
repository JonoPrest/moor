// Bindings for @tanstack/react-virtual (ARCHITECTURE §6.5): one
// virtualizer over `total_rows`, rows outside the cached window are
// placeholders until their chunk arrives.

type virtualItem = {index: int, start: int, size: int, key: string}

type options = {
  count: int,
  getScrollElement: unit => Nullable.t<Dom.element>,
  estimateSize: int => int,
  overscan: int,
}

type virtualizer

@module("@tanstack/react-virtual")
external useVirtualizer: options => virtualizer = "useVirtualizer"

@send external getVirtualItems: virtualizer => array<virtualItem> = "getVirtualItems"
@send external getTotalSize: virtualizer => int = "getTotalSize"
@send external scrollToIndex: (virtualizer, int) => unit = "scrollToIndex"
