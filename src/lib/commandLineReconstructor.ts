// 从逐字符的终端输入流里重组出「用户敲下回车执行的完整命令行」。
//
// 终端输入是逐字符/逐片段的原始字节：普通字符、退格(\x7f)、回车(\r)、Ctrl-C(\x03)、
// 以及方向键/Home/End/Tab 补全等 ESC 转义序列。我们在本地维护一个每会话的输入缓冲：
//   - 可打印字符累积到缓冲
//   - 退格删除末尾字符
//   - 回车提交当前缓冲作为一条命令并清空
//   - Ctrl-C / Ctrl-U 等中断清空缓冲
//
// 关键取舍（宁可漏记，不可记错）：一旦本行出现方向键、Home/End、Tab 补全、Ctrl-R 反向
// 搜索等会让「本地缓冲」与「远端 shell 真实命令行」产生偏差的按键，就把当前行标记为
// tainted（污染）。被污染的行在提交时不作为历史记录返回——因为此时本地缓冲不可信
// （例如用上箭头调出的历史命令、Tab 补全的部分，本地根本没有对应字符）。

const BACKSPACE = '\x7f';
const BACKSPACE_ALT = '\b'; // \x08
const CTRL_C = '\x03';
const CTRL_U = '\x15'; // 清除整行
const CTRL_W = '\x17'; // 删除前一个词——难以精确还原，直接污染
const ESC = '\x1b';
const TAB = '\t';

export interface CommandLineState {
  buffer: string;
  tainted: boolean;
}

export function createCommandLineState(): CommandLineState {
  return { buffer: '', tainted: false };
}

function reset(state: CommandLineState) {
  state.buffer = '';
  state.tainted = false;
}

/**
 * 把一段终端输入喂给状态机。返回本次输入中「提交」出来的完整命令（可能多条，
 * 一段输入里可以包含多个回车）。被污染或空白的行不会出现在返回结果里。
 */
export function feedTerminalInput(state: CommandLineState, data: string): string[] {
  const committed: string[] = [];

  for (let i = 0; i < data.length; i++) {
    const ch = data[i];

    // 回车/换行：提交当前行
    if (ch === '\r' || ch === '\n') {
      // 处理 \r\n：把紧跟的 \n 一并吞掉，避免提交空行
      if (ch === '\r' && data[i + 1] === '\n') {
        i++;
      }
      const cmd = state.buffer.trim();
      if (!state.tainted && cmd.length > 0) {
        committed.push(cmd);
      }
      reset(state);
      continue;
    }

    // 退格
    if (ch === BACKSPACE || ch === BACKSPACE_ALT) {
      state.buffer = state.buffer.slice(0, -1);
      continue;
    }

    // Ctrl-C / Ctrl-U：放弃当前行
    if (ch === CTRL_C || ch === CTRL_U) {
      reset(state);
      continue;
    }

    // Ctrl-W：删词，难以与远端行为对齐，污染本行
    if (ch === CTRL_W) {
      state.tainted = true;
      continue;
    }

    // Tab 补全：本地无法得知补全结果，污染本行
    if (ch === TAB) {
      state.tainted = true;
      continue;
    }

    // ESC 转义序列（方向键、Home/End、功能键、Ctrl-R 搜索结果回填等）：
    // 跳过整个序列并污染本行——这些操作会让本地缓冲与真实命令行不一致。
    if (ch === ESC) {
      state.tainted = true;
      i = skipEscapeSequence(data, i);
      continue;
    }

    // 其它 C0 控制字符（\x00-\x1f，不含上面已处理的）：忽略，不计入命令文本。
    const code = ch.charCodeAt(0);
    if (code < 0x20) {
      continue;
    }

    // 普通可打印字符
    state.buffer += ch;
  }

  return committed;
}

/**
 * 从 escStart（指向 ESC 的位置）开始，跳过一个终端转义序列，返回序列最后一个字符的下标。
 * 支持 CSI(`ESC [ ... 终止字母`)、OSC(`ESC ] ... BEL/ST`)、以及双字符序列(`ESC x`)。
 */
function skipEscapeSequence(data: string, escStart: number): number {
  const next = data[escStart + 1];
  if (next === undefined) return escStart;

  // CSI: ESC [ 参数字节(0x30-0x3f)* 中间字节(0x20-0x2f)* 终止字节(0x40-0x7e)
  if (next === '[') {
    let j = escStart + 2;
    while (j < data.length) {
      const c = data.charCodeAt(j);
      if (c >= 0x40 && c <= 0x7e) return j; // 终止字节
      j++;
    }
    return data.length - 1;
  }

  // OSC: ESC ] ... 以 BEL(\x07) 或 ST(ESC \) 结束
  if (next === ']') {
    let j = escStart + 2;
    while (j < data.length) {
      if (data[j] === '\x07') return j;
      if (data[j] === ESC && data[j + 1] === '\\') return j + 1;
      j++;
    }
    return data.length - 1;
  }

  // 其它双字符序列（如 ESC O A 的 SS3 方向键，ESC 后跟单字母等）
  // SS3: ESC O 终止字母
  if (next === 'O') {
    return escStart + 2 < data.length ? escStart + 2 : data.length - 1;
  }

  // 保守处理：吞掉 ESC 和紧随的一个字符
  return escStart + 1;
}
