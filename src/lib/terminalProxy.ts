/**
 * 终端代理
 * 
 * 为组件提供简化的接口，封装底层 TerminalInstance 的复杂操作
 * 生命周期与组件绑定，不影响底层实例
 */

import type { TerminalInstance } from './terminalInstance';

type TerminalEventHandler = (...args: unknown[]) => void;

export class TerminalProxy {
  private instance: TerminalInstance;
  private localHandlers = new Map<string, Set<TerminalEventHandler>>();

  constructor(instance: TerminalInstance) {
    this.instance = instance;
  }

  /**
   * 获取关联的终端实例
   */
  getInstance(): TerminalInstance {
    return this.instance;
  }

  /**
   * 聚焦终端
   */
  focus(): void {
    this.instance.focus();
  }

  /**
   * 调整终端大小
   */
  fit(): void {
    this.instance.fit();
  }

  /**
   * 写入数据到终端
   * @param data 要写入的数据
   */
  write(data: string): void {
    this.instance.write(data);
  }

  /**
   * 写入换行符
   * @param data 要写入的数据
   */
  writeln(data: string): void {
    this.instance.writeln(data);
  }

  /**
   * 清空终端
   */
  clear(): void {
    this.instance.clear();
  }

  /**
   * 订阅事件（代理级别的事件处理）
   * 事件处理函数与代理绑定，当代理销毁时自动清理
   * @param event 事件名称
   * @param handler 事件处理函数
   */
  on(event: string, handler: TerminalEventHandler): void {
    // 记录本地处理函数
    if (!this.localHandlers.has(event)) {
      this.localHandlers.set(event, new Set());
    }
    this.localHandlers.get(event)!.add(handler);

    // 绑定到实例
    this.instance.on(event, handler);
  }

  /**
   * 取消订阅事件
   * @param event 事件名称
   * @param handler 事件处理函数
   */
  off(event: string, handler: TerminalEventHandler): void {
    // 从本地记录中移除
    const handlers = this.localHandlers.get(event);
    if (handlers) {
      handlers.delete(handler);
    }

    // 从实例解绑
    this.instance.off(event, handler);
  }

  /**
   * 清理所有本地事件处理函数
   */
  clearLocalHandlers(): void {
    this.localHandlers.forEach((handlers, event) => {
      handlers.forEach(handler => {
        this.instance.off(event, handler);
      });
    });
    this.localHandlers.clear();
  }

  /**
   * 销毁代理
   * 注意：这不会销毁底层的 TerminalInstance
   */
  dispose(): void {
    this.clearLocalHandlers();
  }

  /**
   * 检查代理是否有效
   */
  get valid(): boolean {
    return !this.instance.disposed;
  }

  /**
   * 获取终端会话 ID
   */
  get sessionId(): string {
    return this.instance.sessionId;
  }
}
