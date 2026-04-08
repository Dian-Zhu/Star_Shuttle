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

/// 按照 | && || ; 分割命令链（保留每段）
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
            '&' if !in_single && !in_double => {
                if i + 1 < len && chars[i + 1] == '&' {
                    segments.push(&input[start..i]);
                    i += 1;
                    start = i + 1;
                }
            }
            ';' if !in_single && !in_double => {
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
}
