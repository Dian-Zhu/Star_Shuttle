import { type Connection } from './store';

export type PaneId = string;

export interface TerminalPaneNode {
  type: 'pane';
  id: PaneId;
  sessionId: string;
  connection: Connection;
  isRoot?: boolean;
  // Runtime references to existing instances (for root pane)
  existingTerminal?: any;
  existingFitAddon?: any;
  existingSearchAddon?: any;
  onInit?: (term: any, fit: any, search: any) => void;
}

export interface SplitNode {
  type: 'split';
  id: string;
  direction: 'horizontal' | 'vertical';
  splitRatio: number; // 0-1, percentage for the first child
  children: [LayoutNode, LayoutNode];
}

export type LayoutNode = TerminalPaneNode | SplitNode;

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
        // If removeNode returns something different, it means the subtree changed.
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
