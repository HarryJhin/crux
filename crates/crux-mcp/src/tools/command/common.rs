/// Dangerous command patterns that should be rejected
const COMMAND_DENYLIST: &[&str] = &[
    "rm -rf /",
    "rm -rf /*",
    "rm -rf ~",
    "mkfs",
    "dd if=/dev/",
    "dd if= /dev/",
    ":(){ :|:& };:",
    "> /dev/sd",
    "chmod -r 777 /",
    "chmod -rf 777 /",
    "chmod 777 /",
];

/// Validates a command against the denylist
pub(crate) fn validate_command(cmd: &str) -> Result<(), String> {
    let cmd_lower = cmd.to_lowercase();

    for pattern in COMMAND_DENYLIST {
        let pattern_lower = pattern.to_lowercase();
        if cmd_lower.contains(&pattern_lower) {
            return Err(format!(
                "Command rejected: contains dangerous pattern '{}'. This command could cause system damage.",
                pattern
            ));
        }
    }

    Ok(())
}

pub(crate) fn translate_keys(keys: &str) -> String {
    match keys.to_lowercase().as_str() {
        "enter" | "return" => "\n".to_string(),
        "tab" => "\t".to_string(),
        "escape" | "esc" => "\x1b".to_string(),
        "ctrl-c" => "\x03".to_string(),
        "ctrl-d" => "\x04".to_string(),
        "ctrl-z" => "\x1a".to_string(),
        "ctrl-l" => "\x0c".to_string(),
        "ctrl-a" => "\x01".to_string(),
        "ctrl-e" => "\x05".to_string(),
        "ctrl-u" => "\x15".to_string(),
        "ctrl-k" => "\x0b".to_string(),
        "ctrl-w" => "\x17".to_string(),
        "up" => "\x1b[A".to_string(),
        "down" => "\x1b[B".to_string(),
        "right" => "\x1b[C".to_string(),
        "left" => "\x1b[D".to_string(),
        "home" => "\x1b[H".to_string(),
        "end" => "\x1b[F".to_string(),
        "backspace" => "\x7f".to_string(),
        "delete" => "\x1b[3~".to_string(),
        "page-up" => "\x1b[5~".to_string(),
        "page-down" => "\x1b[6~".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate_keys_enter() {
        assert_eq!(translate_keys("enter"), "\n");
        assert_eq!(translate_keys("return"), "\n");
        assert_eq!(translate_keys("ENTER"), "\n");
    }

    #[test]
    fn test_translate_keys_tab() {
        assert_eq!(translate_keys("tab"), "\t");
    }

    #[test]
    fn test_translate_keys_escape() {
        assert_eq!(translate_keys("escape"), "\x1b");
        assert_eq!(translate_keys("esc"), "\x1b");
    }

    #[test]
    fn test_translate_keys_ctrl_sequences() {
        assert_eq!(translate_keys("ctrl-c"), "\x03");
        assert_eq!(translate_keys("ctrl-d"), "\x04");
        assert_eq!(translate_keys("ctrl-z"), "\x1a");
        assert_eq!(translate_keys("ctrl-a"), "\x01");
        assert_eq!(translate_keys("ctrl-e"), "\x05");
    }

    #[test]
    fn test_translate_keys_arrows() {
        assert_eq!(translate_keys("up"), "\x1b[A");
        assert_eq!(translate_keys("down"), "\x1b[B");
        assert_eq!(translate_keys("right"), "\x1b[C");
        assert_eq!(translate_keys("left"), "\x1b[D");
    }

    #[test]
    fn test_translate_keys_home_end() {
        assert_eq!(translate_keys("home"), "\x1b[H");
        assert_eq!(translate_keys("end"), "\x1b[F");
    }

    #[test]
    fn test_translate_keys_backspace_delete() {
        assert_eq!(translate_keys("backspace"), "\x7f");
        assert_eq!(translate_keys("delete"), "\x1b[3~");
    }

    #[test]
    fn test_translate_keys_page_up_down() {
        assert_eq!(translate_keys("page-up"), "\x1b[5~");
        assert_eq!(translate_keys("page-down"), "\x1b[6~");
    }

    #[test]
    fn test_translate_keys_unknown() {
        assert_eq!(translate_keys("x"), "x");
        assert_eq!(translate_keys("unknown-key"), "unknown-key");
    }

    #[test]
    fn test_validate_command_rm_rf_slash() {
        assert!(validate_command("rm -rf /").is_err());
        assert!(validate_command("rm -rf /*").is_err());
        assert!(validate_command("rm -rf ~").is_err());
        assert!(validate_command("sudo rm -rf /").is_err());
    }

    #[test]
    fn test_validate_command_mkfs() {
        assert!(validate_command("mkfs /dev/sda1").is_err());
        assert!(validate_command("sudo mkfs.ext4 /dev/sdb").is_err());
        assert!(validate_command("MKFS /dev/sda").is_err());
    }

    #[test]
    fn test_validate_command_dd() {
        assert!(validate_command("dd if=/dev/zero of=/dev/sda").is_err());
        assert!(validate_command("DD IF=/dev/urandom of=/dev/sdb").is_err());
    }

    #[test]
    fn test_validate_command_fork_bomb() {
        assert!(validate_command(":(){ :|:& };:").is_err());
        assert!(validate_command(":(){ :|:& };: &").is_err());
    }

    #[test]
    fn test_validate_command_direct_disk_write() {
        assert!(validate_command("echo test > /dev/sda").is_err());
        assert!(validate_command("cat file > /dev/sdb").is_err());
        assert!(validate_command("> /dev/sdc").is_err());
    }

    #[test]
    fn test_validate_command_chmod_root() {
        assert!(validate_command("chmod -R 777 /").is_err());
        assert!(validate_command("chmod -rf 777 /").is_err());
        assert!(validate_command("chmod 777 /").is_err());
        assert!(validate_command("CHMOD -R 777 /").is_err());
    }

    #[test]
    fn test_validate_command_safe_commands() {
        assert!(validate_command("ls -la").is_ok());
        assert!(validate_command("echo hello world").is_ok());
        assert!(validate_command("cargo build").is_ok());
        assert!(validate_command("git status").is_ok());
        assert!(validate_command("cd /home/user").is_ok());
        assert!(validate_command("cat file.txt").is_ok());
        assert!(validate_command("mkdir -p /tmp/mydir").is_ok());
        assert!(validate_command("chmod 755 script.sh").is_ok());
        assert!(validate_command("rm -rf ./build").is_ok());
        assert!(validate_command("dd if=input.img of=output.img").is_ok());
    }

    #[test]
    fn test_validate_command_case_insensitive() {
        assert!(validate_command("RM -RF /").is_err());
        assert!(validate_command("Rm -Rf /").is_err());
        assert!(validate_command("rM -rF /").is_err());
    }
}
