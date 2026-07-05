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

/**
 * 基于会话身份收敛「批量移除」后的选中下标。
 *
 * `selectedTerminalIndex` 是对整个 activeTerminals（root 与分屏子会话混排）的
 * 下标。批量 `.filter()` 移除若干会话后，若不同步收敛该下标，会指向错误终端
 * 甚至越界，导致派生的 selectedTerminal 为 null。
 *
 * 规则：
 * - 原选中会话若存活，保持选中（返回其在存活列表中的新位置）；
 * - 原选中会话被移除（或下标本就无效），返回「原下标之前的存活数量」并钳制到
 *   合法范围，等价于就近选中一个仍存在的相邻终端；
 * - 全部移除时返回 0。
 *
 * @param terminals 移除前的完整列表（按显示顺序）
 * @param currentIndex 移除前的选中下标
 * @param removed 被移除的 sessionId 集合
 */
export function computeSelectedIndexAfterBatchRemoval(
  terminals: { sessionId: string }[],
  currentIndex: number,
  removed: Set<string>
): number {
  const survivors = terminals.filter(t => !removed.has(t.sessionId));
  if (survivors.length === 0) return 0;

  const selectedSessionId = terminals[currentIndex]?.sessionId;
  if (selectedSessionId !== undefined && !removed.has(selectedSessionId)) {
    const idx = survivors.findIndex(t => t.sessionId === selectedSessionId);
    if (idx >= 0) return idx;
  }

  let survivorsBefore = 0;
  for (let i = 0; i < currentIndex && i < terminals.length; i++) {
    if (!removed.has(terminals[i].sessionId)) survivorsBefore++;
  }
  return Math.min(survivorsBefore, survivors.length - 1);
}
