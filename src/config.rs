/// Runtime configuration parsed from CLI args (no extra dependencies).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq)]
#[serde(default)]
pub struct Config {
    pub work_mins: u64,
    pub short_mins: u64,
    pub long_mins: u64,
    pub cycles: u64,
    pub auto_start: bool,
    pub muted: bool,
    pub desktop: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            work_mins: 25,
            short_mins: 5,
            long_mins: 15,
            cycles: 4,
            auto_start: false,
            muted: false,
            desktop: true,
        }
    }
}

impl Config {
    pub fn total_focus_secs(&self) -> u64 {
        self.work_mins * 60 * self.cycles
    }
}

pub enum ArgResult {
    Run(Config),
    Help,
    Error(String),
}

fn parse_mins(value: &str, flag: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| format!("{flag} must be a number, got '{value}'"))
        .and_then(|n| {
            if (1..=180).contains(&n) {
                Ok(n)
            } else {
                Err(format!("{flag} must be 1..180 minutes, got {n}"))
            }
        })
}

fn parse_cycles(value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .map_err(|_| format!("--cycles must be a number, got '{value}'"))
        .and_then(|n| {
            if (1..=12).contains(&n) {
                Ok(n)
            } else {
                Err(format!("--cycles must be 1..12, got {n}"))
            }
        })
}

fn take_value(args: &[String], i: &mut usize, flag: &str) -> Result<String, String> {
    *i += 1;
    args.get(*i)
        .cloned()
        .ok_or_else(|| format!("{flag} needs a value, e.g. {flag} 25"))
}

/// Split `--flag=value` into ("--flag", Some("value")) or ("--flag", None).
fn split_eq(arg: &str) -> (&str, Option<&str>) {
    match arg.split_once('=') {
        Some((k, v)) => (k, Some(v)),
        None => (arg, None),
    }
}

pub fn parse_args(raw: Vec<String>) -> ArgResult {
    let mut cfg = Config::default();
    let args: Vec<String> = raw.into_iter().skip(1).collect();
    let mut i = 0;
    while i < args.len() {
        let (flag, inline) = split_eq(&args[i]);
        match flag {
            "-h" | "--help" => return ArgResult::Help,
            "--auto" => cfg.auto_start = true,
            "--no-sound" | "--muted" | "-m" => cfg.muted = true,
            "--no-desktop" => cfg.desktop = false,
            "-w" | "--work" => {
                let v = match inline {
                    Some(v) => v.to_string(),
                    None => match take_value(&args, &mut i, "--work") {
                        Ok(v) => v,
                        Err(e) => return ArgResult::Error(e),
                    },
                };
                match parse_mins(&v, "--work") {
                    Ok(n) => cfg.work_mins = n,
                    Err(e) => return ArgResult::Error(e),
                }
            }
            "-s" | "--short" => {
                let v = match inline {
                    Some(v) => v.to_string(),
                    None => match take_value(&args, &mut i, "--short") {
                        Ok(v) => v,
                        Err(e) => return ArgResult::Error(e),
                    },
                };
                match parse_mins(&v, "--short") {
                    Ok(n) => cfg.short_mins = n,
                    Err(e) => return ArgResult::Error(e),
                }
            }
            "-l" | "--long" => {
                let v = match inline {
                    Some(v) => v.to_string(),
                    None => match take_value(&args, &mut i, "--long") {
                        Ok(v) => v,
                        Err(e) => return ArgResult::Error(e),
                    },
                };
                match parse_mins(&v, "--long") {
                    Ok(n) => cfg.long_mins = n,
                    Err(e) => return ArgResult::Error(e),
                }
            }
            "-c" | "--cycles" => {
                let v = match inline {
                    Some(v) => v.to_string(),
                    None => match take_value(&args, &mut i, "--cycles") {
                        Ok(v) => v,
                        Err(e) => return ArgResult::Error(e),
                    },
                };
                match parse_cycles(&v) {
                    Ok(n) => cfg.cycles = n,
                    Err(e) => return ArgResult::Error(e),
                }
            }
            other => return ArgResult::Error(format!("Unknown option '{other}'. Try --help.")),
        }
        i += 1;
    }
    ArgResult::Run(cfg)
}

pub fn help_text() -> String {
    format!(
        "\
🍅 POMODORO {version} — focus timer with ambient soundscapes

USAGE:
  pomodoro [OPTIONS]

OPTIONS:
  -w, --work <MIN>     Focus length in minutes (1-180)      [default: 25]
  -s, --short <MIN>    Short break in minutes (1-180)       [default: 5]
  -l, --long <MIN>     Long break in minutes (1-180)        [default: 15]
  -c, --cycles <N>     Focus sessions before a long break  [default: 4, 1-12]
      --terminal       Open the terminal interface instead of the desktop window
      --auto           Auto-start next phase, no 3s pause
  -m, --muted --no-sound  Start with the notification bell muted
      --no-desktop   Disable desktop toast notifications
  -h, --help           Show this help

CONTROLS (while running):
  Space   pause / resume    P  pause      R  resume
  S       skip phase        X  restart phase
  M       mute / unmute     Q or Esc  quit

EXAMPLES:
  pomodoro
  pomodoro --work 50 --short 10 --long 30 --cycles 3
  pomodoro -w 25 -s 5 -l 15 -c 4 --auto
",
        version = env!("CARGO_PKG_VERSION")
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn defaults_when_no_args() {
        match parse_args(args(&["pomodoro"])) {
            ArgResult::Run(c) => {
                assert_eq!(c.work_mins, 25);
                assert_eq!(c.cycles, 4);
            }
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn parses_all_flags() {
        match parse_args(args(&[
            "pomodoro",
            "--work",
            "50",
            "--short=10",
            "-l",
            "30",
            "-c",
            "3",
            "--auto",
        ])) {
            ArgResult::Run(c) => {
                assert_eq!(
                    (c.work_mins, c.short_mins, c.long_mins, c.cycles),
                    (50, 10, 30, 3)
                );
                assert!(c.auto_start);
            }
            _ => panic!("expected Run"),
        }
    }

    #[test]
    fn rejects_bad_values() {
        assert!(matches!(
            parse_args(args(&["pomodoro", "--work", "0"])),
            ArgResult::Error(_)
        ));
        assert!(matches!(
            parse_args(args(&["pomodoro", "--bogus"])),
            ArgResult::Error(_)
        ));
    }
}
