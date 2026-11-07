//! Makes `breakbar://start` and friends work, and reads commands from the command line.
use breakbar_core::UrlCommand;

/// Every argument that is a `breakbar://` URL or a `--command` flag, in order.
/// `args` should not include the program name.
pub fn commands_from_args(args: &[String]) -> Vec<UrlCommand> {
    args.iter()
        .filter_map(|a| {
            UrlCommand::from_url(a).or_else(|| a.strip_prefix("--").and_then(UrlCommand::from_host))
        })
        .collect()
}

/// Registers the `breakbar://` scheme for the current user (no admin rights needed).
/// Windows then starts this exe with the URL as an argument, and single-instance hands it to the running app.
#[cfg(windows)]
pub fn register_scheme() {
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;
    let Ok(exe) = std::env::current_exe() else { return };
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let _ = (|| -> std::io::Result<()> {
        let (key, _) = hkcu.create_subkey("Software\\Classes\\breakbar")?;
        key.set_value("", &"URL:BreakBar")?;
        key.set_value("URL Protocol", &"")?;
        let (cmd, _) = key.create_subkey("shell\\open\\command")?;
        cmd.set_value("", &format!("\"{}\" \"%1\"", exe.display()))?;
        Ok(())
    })();
}

#[cfg(not(windows))]
pub fn register_scheme() {}

#[cfg(test)]
mod tests {
    use super::*;
    use breakbar_core::TimerEvent;

    #[test]
    fn reads_urls_and_flags_and_ignores_the_rest() {
        let args: Vec<String> = ["breakbar://startRest", "--pause", "--nonsense", "C:\\x.txt", "BreakBar://TOGGLE"]
            .iter()
            .map(|s| s.to_string())
            .collect();
        assert_eq!(
            commands_from_args(&args),
            vec![
                UrlCommand::Event(TimerEvent::StartRest),
                UrlCommand::Event(TimerEvent::Pause),
                UrlCommand::Toggle
            ]
        );
    }
}
