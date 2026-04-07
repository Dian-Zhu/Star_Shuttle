use crate::modules::ai::command_parser::{detect_injection, parse_statement};
use serde::{Deserialize, Serialize};

/// 沙箱模式
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SandboxMode {
    /// 标准沙箱：白名单放行，其余拦截请求确认
    Standard,
    /// Full 模式：关闭沙箱，不做命令限制
    Full,
}

impl Default for SandboxMode {
    fn default() -> Self {
        SandboxMode::Standard
    }
}

/// 风险级别
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RiskLevel {
    /// 高危：可能造成不可逆损坏
    Critical,
    /// 敏感：需要谨慎
    High,
    /// 一般确认
    Medium,
}

/// 权限判定结果
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum SandboxVerdict {
    /// 允许直接执行
    Allow,
    /// 需要用户二次确认
    NeedConfirm {
        reason: String,
        risk_level: RiskLevel,
        /// 具体触发拦截的命令段
        matched_command: String,
    },
    /// 拒绝执行（注入攻击等）
    Deny { reason: String },
}

// ── 标准沙箱白名单 ────────────────────────────────────────────────────────────

/// 安全的只读/查询命令（标准沙箱自动放行）
const STANDARD_WHITELIST: &[&str] = &[
    // 文件系统查看
    "ls", "ll", "la", "dir", "cat", "head", "tail", "less", "more", "file",
    "stat", "du", "df", "find", "locate", "which", "whereis", "tree",
    // 文本处理
    "grep", "egrep", "fgrep", "rg", "awk", "sed", "cut", "sort", "uniq",
    "wc", "tr", "diff", "comm", "join", "paste", "column", "jq", "yq",
    "echo", "printf", "tee",
    // 系统信息
    "pwd", "whoami", "id", "hostname", "uname", "date", "uptime", "w",
    "who", "last", "lastlog", "env", "printenv", "set",
    // 进程查看
    "ps", "top", "htop", "pgrep", "pstree", "lsof",
    // 网络查看
    "netstat", "ss", "ip", "ifconfig", "arp", "route", "ping", "traceroute",
    "tracepath", "mtr", "dig", "nslookup", "host", "nmap", "curl", "wget",
    // 日志查看
    "journalctl", "dmesg", "syslog",
    // 包管理（只读）
    "apt-cache", "yum", "dnf", "rpm", "dpkg",
    // 容器（只读）
    "docker", "podman", "kubectl", "helm",
    // 其他工具
    "man", "info", "help", "type", "history", "alias",
    "openssl", "base64", "md5sum", "sha256sum",
    "systemctl", "service",
    // 编辑器（查看模式）
    "vim", "vi", "nano", "less",
    // 性能
    "vmstat", "iostat", "sar", "free", "nproc",
];

/// 标准沙箱中需要确认的命令子命令（docker rm 等）
const STANDARD_CONFIRM_SUBCMDS: &[(&str, &str, RiskLevel)] = &[
    ("docker", "rm", RiskLevel::High),
    ("docker", "stop", RiskLevel::High),
    ("docker", "kill", RiskLevel::High),
    ("docker", "rmi", RiskLevel::High),
    ("docker", "prune", RiskLevel::High),
    ("kubectl", "delete", RiskLevel::Critical),
    ("kubectl", "drain", RiskLevel::Critical),
    ("kubectl", "cordon", RiskLevel::High),
];

// ── 严格沙箱黑名单 ────────────────────────────────────────────────────────────

/// 高危命令（Critical - 严格沙箱必须确认）
const STRICT_CRITICAL: &[&str] = &[
    "mkfs", "mkswap", "fdisk", "parted", "gdisk", "blkdiscard",
    "dd", "shred",
    "halt", "poweroff", "shutdown", "reboot", "init",
    ":(){ :|:& };:",  // fork bomb
    "userdel", "deluser",
];

/// 敏感命令（High - 严格沙箱需要确认）
const STRICT_HIGH: &[&str] = &[
    "rm", "rmdir", "unlink",
    "chmod", "chown", "chgrp", "chattr",
    "mv", "cp",  // 覆盖时危险
    "kill", "pkill", "killall",
    "mount", "umount",
    "iptables", "ip6tables", "nftables", "firewall-cmd", "ufw",
    "useradd", "usermod", "adduser", "passwd",
    "visudo", "sudoers",
    "crontab",
    "at",
    "pip", "pip3", "npm", "yarn", "cargo", "gem", "go",
    "apt", "apt-get", "yum", "dnf", "pacman", "brew",
    "systemctl", "service",
    "ln",
    "truncate",
    "mkfifo",
];

/// 严格沙箱：特定参数使普通命令升级为 Critical
const STRICT_CRITICAL_PATTERNS: &[(&str, &[&str])] = &[
    ("rm", &["-rf", "-fr", "--no-preserve-root"]),
    ("chmod", &["777", "a+x", "o+w"]),
    ("dd", &["if=/dev/", "of=/dev/"]),
    ("curl", &["|", "bash", "sh"]),
    ("wget", &["-O-", "|"]),
    ("python", &["-c"]),
    ("python3", &["-c"]),
    ("bash", &["-c"]),
    ("sh", &["-c"]),
    ("eval", &[]),
    ("exec", &[]),
];

// ── 判定引擎 ──────────────────────────────────────────────────────────────────

pub struct Sandbox {
    pub mode: SandboxMode,
}

impl Sandbox {
    pub fn new(mode: SandboxMode) -> Self {
        Self { mode }
    }

    /// 对整条命令字符串进行权限判定
    pub fn check(&self, command: &str) -> SandboxVerdict {
        if self.mode == SandboxMode::Full {
            return SandboxVerdict::Allow;
        }

        // 1. 标准模式下执行注入检测
        if detect_injection(command) {
            return SandboxVerdict::Deny {
                reason: "检测到潜在命令注入（$()、反引号等），已拒绝执行".to_string(),
            };
        }

        let stmt = parse_statement(command);

        match self.mode {
            SandboxMode::Standard => self.check_standard(&stmt, command),
            SandboxMode::Full => SandboxVerdict::Allow,
        }
    }

    // ── 标准沙箱（白名单） ──────────────────────────────────────────────────

    fn check_standard(&self, stmt: &crate::modules::ai::command_parser::ParsedStatement, _raw: &str) -> SandboxVerdict {
        for cmd in &stmt.commands {
            let name = cmd.name.as_str();

            // 检查是否在白名单中
            if STANDARD_WHITELIST.contains(&name) {
                // 检查特殊子命令（如 docker rm）
                if let Some(verdict) = self.check_standard_subcmd(cmd) {
                    return verdict;
                }
                // 白名单命令放行
                continue;
            }

            // 不在白名单：需要确认
            return SandboxVerdict::NeedConfirm {
                reason: format!(
                    "命令 `{}` 不在安全白名单中，需要确认后执行",
                    name
                ),
                risk_level: RiskLevel::Medium,
                matched_command: cmd.raw.clone(),
            };
        }
        SandboxVerdict::Allow
    }

    fn check_standard_subcmd(&self, cmd: &crate::modules::ai::command_parser::ParsedCommand) -> Option<SandboxVerdict> {
        let name = cmd.name.as_str();
        for (base, sub, level) in STANDARD_CONFIRM_SUBCMDS {
            if name == *base {
                if cmd.args.first().map(|a| a.as_str()) == Some(sub) {
                    return Some(SandboxVerdict::NeedConfirm {
                        reason: format!("`{} {}` 是破坏性操作，需要确认", base, sub),
                        risk_level: *level,
                        matched_command: cmd.raw.clone(),
                    });
                }
            }
        }
        None
    }

    // ── 严格沙箱（黑名单） ──────────────────────────────────────────────────

    fn check_strict(&self, stmt: &crate::modules::ai::command_parser::ParsedStatement, _raw: &str) -> SandboxVerdict {
        for cmd in &stmt.commands {
            let name = cmd.name.as_str();

            // Critical 黑名单
            if STRICT_CRITICAL.contains(&name) {
                return SandboxVerdict::NeedConfirm {
                    reason: format!("`{}` 是高危命令，可能造成不可逆损坏", name),
                    risk_level: RiskLevel::Critical,
                    matched_command: cmd.raw.clone(),
                };
            }

            // High 黑名单
            if STRICT_HIGH.contains(&name) {
                // 再检查是否有让它升级为 Critical 的参数
                if let Some(v) = self.check_critical_args(cmd) {
                    return v;
                }
                return SandboxVerdict::NeedConfirm {
                    reason: format!("`{}` 是敏感命令，需要确认", name),
                    risk_level: RiskLevel::High,
                    matched_command: cmd.raw.clone(),
                };
            }

            // 检查参数模式（升级 Critical）
            if let Some(v) = self.check_critical_args(cmd) {
                return v;
            }
        }
        SandboxVerdict::Allow
    }

    fn check_critical_args(&self, cmd: &crate::modules::ai::command_parser::ParsedCommand) -> Option<SandboxVerdict> {
        let name = cmd.name.as_str();
        for (base, danger_args) in STRICT_CRITICAL_PATTERNS {
            if name == *base {
                if danger_args.is_empty() {
                    // 仅命令名匹配即为 Critical
                    return Some(SandboxVerdict::NeedConfirm {
                        reason: format!("`{}` 是极高风险命令，需要确认", name),
                        risk_level: RiskLevel::Critical,
                        matched_command: cmd.raw.clone(),
                    });
                }
                for danger_arg in *danger_args {
                    if cmd.args.iter().any(|a| a.contains(danger_arg)) {
                        return Some(SandboxVerdict::NeedConfirm {
                            reason: format!(
                                "`{}` 带有危险参数 `{}`，可能造成不可逆损坏",
                                name, danger_arg
                            ),
                            risk_level: RiskLevel::Critical,
                            matched_command: cmd.raw.clone(),
                        });
                    }
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_standard_allow() {
        let sb = Sandbox::new(SandboxMode::Standard);
        assert!(matches!(sb.check("ls -la /tmp"), SandboxVerdict::Allow));
        assert!(matches!(sb.check("ps aux | grep nginx"), SandboxVerdict::Allow));
        assert!(matches!(sb.check("cat /etc/hosts"), SandboxVerdict::Allow));
    }

    #[test]
    fn test_standard_need_confirm() {
        let sb = Sandbox::new(SandboxMode::Standard);
        assert!(matches!(sb.check("rm -rf /tmp/test"), SandboxVerdict::NeedConfirm { .. }));
        assert!(matches!(sb.check("apt install nginx"), SandboxVerdict::NeedConfirm { .. }));
    }

    #[test]
    fn test_full_allow() {
        let sb = Sandbox::new(SandboxMode::Full);
        assert!(matches!(sb.check("rm -rf /"), SandboxVerdict::Allow));
        assert!(matches!(sb.check("mkfs.ext4 /dev/sda"), SandboxVerdict::Allow));
    }

    #[test]
    fn test_full_allows_injection_patterns() {
        let sb = Sandbox::new(SandboxMode::Full);
        assert!(matches!(sb.check("ls $(cat /etc/shadow)"), SandboxVerdict::Allow));
    }

    #[test]
    fn test_standard_allow_safe() {
        let sb = Sandbox::new(SandboxMode::Standard);
        assert!(matches!(sb.check("ls -la"), SandboxVerdict::Allow));
        assert!(matches!(sb.check("cat /etc/passwd"), SandboxVerdict::Allow));
    }

    #[test]
    fn test_injection_denied() {
        let sb = Sandbox::new(SandboxMode::Standard);
        assert!(matches!(sb.check("ls $(cat /etc/shadow)"), SandboxVerdict::Deny { .. }));
    }
}
