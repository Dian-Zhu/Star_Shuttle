/**
 * 全局终端实例池
 * 
 * 负责管理所有终端实例的生命周期
 * 终端实例的生命周期独立于组件生命周期
 * 组件通过 sessionId 获取实例，不影响实例的生命周期
 */

import type { TerminalInstance } from './terminalInstance';
import type { TerminalProxy } from './terminalProxy';
import { TerminalInstance as TerminalInstanceClass } from './terminalInstance';
import { TerminalProxy as TerminalProxyClass } from './terminalProxy';

export class TerminalPool {
  private static instance: TerminalPool;

  private instances = new Map<string, TerminalInstance>();
  private proxies = new Map<string, TerminalProxy>();

  private constructor() {}

  static getInstance(): TerminalPool {
    if (!TerminalPool.instance) {
      TerminalPool.instance = new TerminalPool();
    }
    return TerminalPool.instance;
  }

  /**
   * 注册已经初始化好的终端实例
   * @param instance 已经初始化好的终端实例
   */
  registerInstance(instance: TerminalInstance): void {
    const sessionId = instance.sessionId;
    
    // 如果已存在，先销毁旧实例
    if (this.instances.has(sessionId)) {
      console.warn(`[TerminalPool] Replacing existing instance for session ${sessionId}`);
      const oldInstance = this.instances.get(sessionId)!;
      oldInstance.dispose();
    }

    this.instances.set(sessionId, instance);
    console.log(`[TerminalPool] Registered terminal instance for session ${sessionId}`);
  }

  /**
   * 获取或创建终端实例
   * 如果实例已存在，返回现有实例（不会重复创建）
   * @param sessionId 会话 ID
   * @param options 终端选项（仅在创建新实例时使用）
   * @returns 终端实例
   */
  getInstance(sessionId: string, options?: any): TerminalInstance {
    // 如果实例已存在，直接返回
    if (this.instances.has(sessionId)) {
      return this.instances.get(sessionId)!;
    }

    // 创建新实例
    const instance = new TerminalInstanceClass(sessionId, options);
    this.instances.set(sessionId, instance);

    console.log(`[TerminalPool] Created new terminal instance for session ${sessionId}`);
    
    return instance;
  }

  /**
   * 获取或创建终端代理
   * 代理与组件生命周期绑定，用于简化组件的交互
   * @param sessionId 会话 ID
   * @param options 终端选项（仅在创建新实例时使用）
   * @returns 终端代理
   */
  getProxy(sessionId: string, options?: any): TerminalProxy {
    // 如果代理已存在，直接返回
    if (this.proxies.has(sessionId)) {
      return this.proxies.get(sessionId)!;
    }

    // 获取或创建实例
    const instance = this.getInstance(sessionId, options);

    // 创建代理
    const proxy = new TerminalProxyClass(instance);
    this.proxies.set(sessionId, proxy);

    return proxy;
  }

  /**
   * 检查实例是否存在
   * @param sessionId 会话 ID
   * @returns 是否存在
   */
  hasInstance(sessionId: string): boolean {
    return this.instances.has(sessionId);
  }

  /**
   * 检查实例是否已挂载
   * @param sessionId 会话 ID
   * @returns 是否已挂载
   */
  isMounted(sessionId: string): boolean {
    const instance = this.instances.get(sessionId);
    return instance ? instance.mounted : false;
  }

  /**
   * 获取实例
   * @param sessionId 会话 ID
   * @returns 终端实例（如果存在）
   */
  retrieveInstance(sessionId: string): TerminalInstance | undefined {
    return this.instances.get(sessionId);
  }

  /**
   * 获取所有活跃的实例
   * @returns 终端实例数组
   */
  getActiveInstances(): TerminalInstance[] {
    return Array.from(this.instances.values());
  }

  /**
   * 获取所有活跃的会话 ID
   * @returns 会话 ID 数组
   */
  getActiveSessionIds(): string[] {
    return Array.from(this.instances.keys());
  }

  /**
   * 获取活跃实例数量
   * @returns 实例数量
   */
  getActiveCount(): number {
    return this.instances.size;
  }

  /**
   * 销毁指定会话的实例和代理
   * @param sessionId 会话 ID
   */
  destroyInstance(sessionId: string): void {
    console.log(`[TerminalPool] Destroying instance for session ${sessionId}`);

    // 销毁代理
    const proxy = this.proxies.get(sessionId);
    if (proxy) {
      proxy.dispose();
      this.proxies.delete(sessionId);
    }

    // 销毁实例
    const instance = this.instances.get(sessionId);
    if (instance) {
      instance.dispose();
      this.instances.delete(sessionId);
    }
  }

  /**
   * 销毁所有实例和代理
   */
  destroyAll(): void {
    console.log(`[TerminalPool] Destroying all instances (${this.instances.size})`);

    // 销毁所有代理
    this.proxies.forEach(proxy => proxy.dispose());
    this.proxies.clear();

    // 销毁所有实例
    this.instances.forEach(instance => instance.dispose());
    this.instances.clear();
  }

  /**
   * 调整所有实例的大小
   */
  fitAll(): void {
    this.instances.forEach(instance => instance.fit());
  }

  /**
   * 获取统计信息
   * @returns 统计信息对象
   */
  getStats(): {
    totalInstances: number;
    totalProxies: number;
    mountedCount: number;
    disposedCount: number;
  } {
    let mountedCount = 0;
    let disposedCount = 0;

    this.instances.forEach(instance => {
      if (instance.mounted) mountedCount++;
      if (instance.disposed) disposedCount++;
    });

    return {
      totalInstances: this.instances.size,
      totalProxies: this.proxies.size,
      mountedCount,
      disposedCount,
    };
  }
}

// 导出单例实例
export const terminalPool = TerminalPool.getInstance();
