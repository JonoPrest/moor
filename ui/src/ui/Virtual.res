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
/// Ref callback for dynamic row heights (inline threads): the element must
/// carry `data-index`.
@get external measureElement: virtualizer => Nullable.t<Dom.element> => unit = "measureElement"
@send external getTotalSize: virtualizer => int = "getTotalSize"
@send external scrollToIndex: (virtualizer, int) => unit = "scrollToIndex"
/// `align` is TanStack's: "auto" keeps the row on screen with the least
/// movement, "start"/"center"/"end" put it there (`z t`/`z z`/`z b`).
@send
external scrollToIndexAligned: (virtualizer, int, {"align": string}) => unit = "scrollToIndex"
