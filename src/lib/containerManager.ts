/**
 * 容器管理器
 * 
 * 负责为终端实例创建和管理 DOM 容器
 * 确保容器干净，避免 DOM 污染
 */

export class ContainerManager {
  private static instance: ContainerManager;
  private static readonly XTERM_DOM_SELECTORS =
    '.xterm, .xterm-viewport, .xterm-screen, .xterm-helpers, .xterm-helper-textarea';

  private constructor() {}

  static getInstance(): ContainerManager {
    if (!ContainerManager.instance) {
      ContainerManager.instance = new ContainerManager();
    }
    return ContainerManager.instance;
  }

  /**
   * 为终端实例创建容器
   * @returns 新的容器元素
   */
  createContainer(): HTMLElement {
    const container = document.createElement('div');
    container.className = 'terminal-container';
    container.style.width = '100%';
    container.style.height = '100%';
    container.style.position = 'relative';
    container.style.overflow = 'hidden';
    
    return container;
  }

  /**
   * 清理容器内容
   * @param container 要清理的容器
   */
  cleanupContainer(container: HTMLElement): void {
    if (!container) return;

    // 移除所有子元素
    while (container.firstChild) {
      container.removeChild(container.firstChild);
    }

    // 清除所有内联样式（保留基本布局样式）
    const keepStyles = ['width', 'height', 'position', 'overflow'];
    const computedStyle = window.getComputedStyle(container);
    
    keepStyles.forEach(prop => {
      const value = computedStyle.getPropertyValue(prop);
      if (value) {
        container.style.setProperty(prop, value);
      }
    });

    // 清除其他所有样式
    Array.from(container.style)
      .filter(prop => !keepStyles.includes(prop))
      .forEach(prop => {
        container.style.removeProperty(prop);
      });
  }

  /**
   * 确保容器干净
   * @param container 要检查和清理的容器
   */
  ensureClean(container: HTMLElement): void {
    if (!container) return;

    // 检查容器是否有 xterm 相关类名
    if (container.classList.contains('xterm')) {
      this.cleanupContainer(container);
    }

    // 检查是否有 xterm 相关的子元素
    const xtermElements = container.querySelectorAll(ContainerManager.XTERM_DOM_SELECTORS);
    if (xtermElements.length > 0) {
      this.cleanupContainer(container);
    }
  }

  /**
   * 安全地追加子元素
   * @param container 父容器
   * @param child 子元素
   */
  safeAppend(container: HTMLElement, child: HTMLElement): void {
    if (!container || !child) return;

    // 确保容器干净
    this.ensureClean(container);

    // 追加子元素
    container.appendChild(child);
  }

  /**
   * 安全地替换子元素
   * @param container 父容器
   * @param oldChild 旧子元素
   * @param newChild 新子元素
   */
  safeReplace(container: HTMLElement, oldChild: HTMLElement, newChild: HTMLElement): void {
    if (!container || !newChild) return;

    if (oldChild && oldChild.parentNode === container) {
      container.replaceChild(newChild, oldChild);
    } else {
      this.safeAppend(container, newChild);
    }
  }
}

// 导出单例实例
export const containerManager = ContainerManager.getInstance();
