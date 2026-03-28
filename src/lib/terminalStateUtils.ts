export function computeNextSelectedIndexAfterRemoval(
  currentIndex: number,
  removedIndex: number,
  previousLength: number
): number {
  const nextLength = Math.max(0, previousLength - 1);
  if (nextLength === 0) return 0;

  if (removedIndex < currentIndex) {
    return Math.max(0, currentIndex - 1);
  }

  if (removedIndex === currentIndex) {
    return Math.min(currentIndex, nextLength - 1);
  }

  return Math.min(currentIndex, nextLength - 1);
}
