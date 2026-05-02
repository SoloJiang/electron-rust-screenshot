#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Modifiers {
    pub cmd: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

pub fn parse(modifiers: &[String]) -> Modifiers {
    let mut m = Modifiers {
        cmd: false,
        ctrl: false,
        shift: false,
        alt: false,
    };
    for raw in modifiers {
        match raw.as_str() {
            "Cmd" | "Meta" | "Super" => m.cmd = true,
            "Ctrl" => m.ctrl = true,
            "Shift" => m.shift = true,
            "Alt" | "Option" => m.alt = true,
            _ => {}
        }
    }
    m
}

// ------------------------------------------------------------------
// Real-tier modifier mapping (cliclick / platform key names)
// ------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modifier {
    Cmd,
    Ctrl,
    Shift,
    Alt,
}

pub fn parse_one(name: &str) -> Option<Modifier> {
    match name.to_ascii_lowercase().as_str() {
        "cmd" | "meta" | "super" => Some(Modifier::Cmd),
        "ctrl" | "control" => Some(Modifier::Ctrl),
        "shift" => Some(Modifier::Shift),
        "alt" | "option" => Some(Modifier::Alt),
        _ => None,
    }
}

pub fn parse_all(names: &[String]) -> Vec<Modifier> {
    names.iter().filter_map(|n| parse_one(n)).collect()
}

#[cfg(target_os = "macos")]
pub fn cliclick_name(m: Modifier) -> &'static str {
    match m {
        Modifier::Cmd => "cmd",
        Modifier::Ctrl => "ctrl",
        Modifier::Shift => "shift",
        Modifier::Alt => "alt",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_flag_aliases() {
        let m = parse(&["Cmd".into(), "Shift".into()]);
        assert!(m.cmd);
        assert!(m.shift);
        assert!(!m.ctrl);
    }

    #[test]
    fn modifier_enum_parses_aliases() {
        assert_eq!(parse_one("Cmd"), Some(Modifier::Cmd));
        assert_eq!(parse_one("meta"), Some(Modifier::Cmd));
        assert_eq!(parse_one("super"), Some(Modifier::Cmd));
        assert_eq!(parse_one("Option"), Some(Modifier::Alt));
        assert_eq!(parse_one("control"), Some(Modifier::Ctrl));
    }

    #[test]
    fn unknown_returns_none() {
        assert_eq!(parse_one("Hyper"), None);
    }
}
