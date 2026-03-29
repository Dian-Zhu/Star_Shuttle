import { type Connection } from './store';
import type { TerminalProxy } from './terminalProxy';

export type PaneId = string;

export interface TerminalPaneNode {
  type: 'pane';
  id: PaneId;
  sessionId: string;
  connection: Connection;
  isRoot?: boolean;
  createdAt: number;
  onInit?: (proxy: TerminalProxy) => void;
}

export interface SplitNode {
  type: 'split';
  id: string;
  direction: 'horizontal' | 'vertical';
  splitRatio: number; // 0-1, percentage for the first child
  children: [LayoutNode, LayoutNode];
}

export type LayoutNode = TerminalPaneNode | SplitNode;

export function getSplitDirectionFromDrag(
  initialDirection: SplitNode['direction'],
  start: { x: number; y: number },
  current: { x: number; y: number },
  threshold = 48
): SplitNode['direction'] {
  const dx = current.x - start.x;
  const dy = current.y - start.y;

  if (
    initialDirection === 'vertical' &&
    Math.abs(dy) >= threshold &&
    Math.abs(dy) > Math.abs(dx) + 16
  ) {
    return 'horizontal';
  }

  if (
    initialDirection === 'horizontal' &&
    Math.abs(dx) >= threshold &&
    Math.abs(dx) > Math.abs(dy) + 16
  ) {
    return 'vertical';
  }

  return initialDirection;
}

// Helper to generate unique IDs
export function generateId(): string {
  return Math.random().toString(36).substring(2, 9);
}

// Helper to find a node by ID path (optional, might not need if we just traverse)
export function findNode(root: LayoutNode, id: string): LayoutNode | null {
  if (root.id === id) return root;
  if (root.type === 'split') {
    return findNode(root.children[0], id) || findNode(root.children[1], id);
  }
  return null;
}

// Helper to find a node by Session ID
export function findNodeBySessionId(root: LayoutNode, sessionId: string): LayoutNode | null {
  if (root.type === 'pane' && root.sessionId === sessionId) return root;
  if (root.type === 'split') {
    return findNodeBySessionId(root.children[0], sessionId) || findNodeBySessionId(root.children[1], sessionId);
  }
  return null;
}

// Helper to replace a node in the tree
export function replaceNode(root: LayoutNode, targetId: string, newNode: LayoutNode): LayoutNode {
  if (root.id === targetId) {
    return newNode;
  }
  if (root.type === 'split') {
    return {
      ...root,
      children: [
        replaceNode(root.children[0], targetId, newNode),
        replaceNode(root.children[1], targetId, newNode)
      ]
    };
  }
  return root;
}

// Helper to remove a node and return its sibling (promoting it)
// Returns null if the root itself is removed
export function removeNode(root: LayoutNode, targetId: string): LayoutNode | null {
  if (root.id === targetId) {
    return null; 
  }
  
  if (root.type === 'split') {
    // Check if one of the children is the target
    if (root.children[0].id === targetId) {
      return root.children[1]; // Promote sibling
    }
    if (root.children[1].id === targetId) {
      return root.children[0]; // Promote sibling
    }
    
    // Recursive search
    const newLeft = removeNode(root.children[0], targetId);
    const newRight = removeNode(root.children[1], targetId);
    
    // If a child structure changed (but wasn't the direct target being removed), update it
    if (newLeft !== root.children[0]) {
        // If the recursive call returned null, it means the child [0] was the target (handled above)
        // OR the child [0] was a subtree that collapsed to null (shouldn't happen with promotion logic usually, unless empty)
        // With promotion logic:
        // If removeNode returns something different, it means that subtree changed.
        return {
            ...root,
            children: [newLeft as LayoutNode, root.children[1]]
        };
    }
    
    if (newRight !== root.children[1]) {
        return {
            ...root,
            children: [root.children[0], newRight as LayoutNode]
        };
    }
  }

  return root;
}

// Helper to get pane index (position among all panes in the same terminal view)
export function getPaneIndex(root: LayoutNode, targetId: string): number {
  const allPanes: { id: string; createdAt: number }[] = [];

  function collectPanes(node: LayoutNode) {
    if (node.type === 'pane') {
      allPanes.push({ id: node.id, createdAt: node.createdAt });
    } else if (node.type === 'split') {
      collectPanes(node.children[0]);
      collectPanes(node.children[1]);
    }
  }

  collectPanes(root);
  
  // Sort by createdAt to ensure numbering follows creation order
  allPanes.sort((a, b) => a.createdAt - b.createdAt);
  
  return allPanes.findIndex(p => p.id === targetId) + 1; // Return 1-based index
}
