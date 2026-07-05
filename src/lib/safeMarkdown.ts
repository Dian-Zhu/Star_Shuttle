import { marked } from 'marked';
import DOMPurify from 'dompurify';

/**
 * 将 markdown 渲染为“已净化”的 HTML，供 `{@html}` 安全注入。
 *
 * 背景：AI 回复与远程主机命令输出都会经 marked 渲染后以 `{@html}` 注入 DOM。
 * marked 默认不转义原始 HTML，若内容含 `<img src=x onerror=...>`、`<script>`
 * 等标签，会在 Tauri webview 内执行脚本（XSS），进而可桥接到已注册的 invoke
 * 命令。因此所有 markdown→HTML 的结果都必须先经 DOMPurify 净化，去除脚本、
 * 事件处理属性和危险协议，再交给 `{@html}`。
 *
 * 统一入口，避免各组件各自调用 marked 时漏掉净化步骤。
 */
export function renderMarkdownSafe(content: string): string {
  if (!content) {
    return '';
  }
  try {
    const raw = marked.parse(content, { async: false, gfm: true, breaks: true }) as string;
    return DOMPurify.sanitize(raw, {
      // 禁止事件处理属性（onerror/onload 等）由 DOMPurify 默认移除；
      // 这里额外禁用 data URI 之外的危险协议由默认策略覆盖。
      USE_PROFILES: { html: true },
    });
  } catch {
    // 解析失败时退回纯文本转义，绝不返回未净化的原始 HTML。
    return DOMPurify.sanitize(content);
  }
}
