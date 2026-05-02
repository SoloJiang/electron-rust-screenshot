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
