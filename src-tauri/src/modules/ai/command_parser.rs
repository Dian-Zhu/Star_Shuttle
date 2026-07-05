/// Shell 命令解析器
/// 将 shell 命令字符串解析为可检查的结构，用于沙箱权限判定

#[derive(Debug, Clone)]
pub struct ParsedCommand {
    /// 主命令名（如 "rm"、"docker"）
    pub name: String,
    /// 完整参数列表
    pub args: Vec<String>,
    /// 原始命令字符串
    pub raw: String,
}

/// 一条 shell 语句可能包含多个命令（管道 / && / || / ;）
#[derive(Debug, Clone)]
pub struct ParsedStatement {
    pub commands: Vec<ParsedCommand>,
    pub raw: String,
}

/// 解析 shell 语句，将整行拆分为各个命令段
pub fn parse_statement(input: &str) -> ParsedStatement {
    let raw = input.to_string();
    let commands = split_commands(input)
        .into_iter()
        .filter_map(|seg| parse_single(seg.trim()))
        .collect();
    ParsedStatement { commands, raw }
}

/// 按照命令分隔符分割命令链（保留每段）
///
/// 分隔符包括：管道 `|` `||`、逻辑与 `&&`、后台符 `&`、分号 `;`、
/// 换行 `\n`，以及重定向 `>` `>>` `<`。重定向被视为分隔符，
/// 以便其目标（如 `> /etc/cron.d/x`）作为独立命令段参与白名单判定，
/// 避免 `echo x > /path` 这类写文件操作被首词 `echo` 蒙混过关。
fn split_commands(input: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut start = 0;
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut in_single = false;
    let mut in_double = false;

    while i < len {
        match chars[i] {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            '|' if !in_single && !in_double => {
                segments.push(&input[start..i]);
                // skip '||' or '|'
                if i + 1 < len && chars[i + 1] == '|' {
                    i += 1;
                }
                start = i + 1;
            }
            // 单个 `&`（后台符）与 `&&`（逻辑与）都作为分隔符
            '&' if !in_single && !in_double => {
                segments.push(&input[start..i]);
                if i + 1 < len && chars[i + 1] == '&' {
                    i += 1;
                }
                start = i + 1;
            }
            // 重定向 `>` `>>` `<`：目标应作为独立段被检查
            '>' if !in_single && !in_double => {
                segments.push(&input[start..i]);
                if i + 1 < len && chars[i + 1] == '>' {
                    i += 1;
                }
                start = i + 1;
            }
            '<' if !in_single && !in_double => {
                segments.push(&input[start..i]);
                start = i + 1;
            }
            // 分号与换行都终止一条命令
            ';' | '\n' | '\r' if !in_single && !in_double => {
                segments.push(&input[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }
    if start < input.len() {
        segments.push(&input[start..]);
    }
    segments
        .into_iter()
        .filter(|s| !s.trim().is_empty())
        .collect()
}

/// 解析单个命令段（提取命令名和参数）
fn parse_single(segment: &str) -> Option<ParsedCommand> {
    // 简单按空白分词（不处理引号内空格，够用于沙箱检测）
    let tokens: Vec<String> = tokenize(segment);
    if tokens.is_empty() {
        return None;
    }

    // 跳过环境变量赋值（KEY=VALUE cmd）
    let mut idx = 0;
    while idx < tokens.len() {
        if tokens[idx].contains('=') && !tokens[idx].starts_with('-') {
            idx += 1;
        } else {
            break;
        }
    }
    if idx >= tokens.len() {
        return None;
    }

    // 跳过 sudo / env / nice / nohup / time 等包装命令，取真实命令
    let wrapper = ["sudo", "env", "nice", "nohup", "time", "strace", "valgrind"];
    while idx < tokens.len() && wrapper.contains(&tokens[idx].as_str()) {
        idx += 1;
    }
    if idx >= tokens.len() {
        return None;
    }

    let name = tokens[idx].clone();
    let args = tokens[idx + 1..].to_vec();

    Some(ParsedCommand {
        name,
        args,
        raw: segment.to_string(),
    })
}

/// 简单 tokenizer（按空白分词，保留引号内内容）
fn tokenize(s: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_single = false;
    let mut in_double = false;

    for ch in s.chars() {
        match ch {
            '\'' if !in_double => {
                in_single = !in_single;
            }
            '"' if !in_single => {
                in_double = !in_double;
            }
            ' ' | '\t' if !in_single && !in_double => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

/// 检查命令字符串是否包含注入攻击模式
pub fn detect_injection(input: &str) -> bool {
    // 检测命令替换
    if input.contains("$(") || input.contains('`') {
        return true;
    }
    // 检测 null byte
    if input.contains('\0') {
        return true;
    }
    // 检测反斜杠换行（绕过技巧）
    if input.contains("\\\n") {
        return true;
    }
    // 检测裸换行：一行命令内不应出现换行，出现即视为多命令拼接的绕过尝试。
    // split_commands 已按换行拆分，此处作为纵深防御，拦在授权判定之前。
    if input.contains('\n') || input.contains('\r') {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_command() {
        let stmt = parse_statement("ls -la /tmp");
        assert_eq!(stmt.commands.len(), 1);
        assert_eq!(stmt.commands[0].name, "ls");
        assert_eq!(stmt.commands[0].args, vec!["-la", "/tmp"]);
    }

    #[test]
    fn test_pipe() {
        let stmt = parse_statement("ps aux | grep nginx");
        assert_eq!(stmt.commands.len(), 2);
        assert_eq!(stmt.commands[0].name, "ps");
        assert_eq!(stmt.commands[1].name, "grep");
    }

    #[test]
    fn test_and_chain() {
        let stmt = parse_statement("cd /tmp && rm -rf test");
        assert_eq!(stmt.commands.len(), 2);
        assert_eq!(stmt.commands[1].name, "rm");
    }

    #[test]
    fn test_sudo_unwrap() {
        let stmt = parse_statement("sudo rm -rf /");
        assert_eq!(stmt.commands[0].name, "rm");
    }

    #[test]
    fn test_injection_detection() {
        assert!(detect_injection("ls $(cat /etc/passwd)"));
        assert!(detect_injection("ls `whoami`"));
        assert!(!detect_injection("ls -la /tmp"));
    }

    #[test]
    fn test_background_single_ampersand_splits() {
        // 单个 `&` 后台符必须切分，否则 `rm -rf /tmp` 会沦为 `ls` 的参数
        let stmt = parse_statement("ls & rm -rf /tmp");
        assert_eq!(stmt.commands.len(), 2);
        assert_eq!(stmt.commands[0].name, "ls");
        assert_eq!(stmt.commands[1].name, "rm");
    }

    #[test]
    fn test_newline_splits() {
        let stmt = parse_statement("ls\nrm -rf /tmp");
        assert_eq!(stmt.commands.len(), 2);
        assert_eq!(stmt.commands[0].name, "ls");
        assert_eq!(stmt.commands[1].name, "rm");
    }

    #[test]
    fn test_redirect_target_is_separate_segment() {
        // `echo x > /etc/cron.d/x` 的重定向目标应作为独立段，不被首词 echo 蒙混
        let stmt = parse_statement("echo x > /etc/cron.d/x");
        assert_eq!(stmt.commands.len(), 2);
        assert_eq!(stmt.commands[0].name, "echo");
        assert_eq!(stmt.commands[1].name, "/etc/cron.d/x");
    }

    #[test]
    fn test_append_redirect_splits() {
        let stmt = parse_statement("echo KEY >> /root/.ssh/authorized_keys");
        assert_eq!(stmt.commands.len(), 2);
        assert_eq!(stmt.commands[1].name, "/root/.ssh/authorized_keys");
    }

    #[test]
    fn test_ampersand_not_split_inside_quotes() {
        let stmt = parse_statement("echo 'a & b'");
        assert_eq!(stmt.commands.len(), 1);
        assert_eq!(stmt.commands[0].name, "echo");
    }

    #[test]
    fn test_injection_detects_bare_newline() {
        assert!(detect_injection("ls\nrm -rf /"));
        assert!(detect_injection("ls\r\nrm -rf /"));
    }
}
