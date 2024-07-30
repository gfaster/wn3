use std::{path::Path, str::FromStr, sync::Mutex};

use anyhow::anyhow;

static LOCK: Mutex<()> = Mutex::new(());

macro_rules! error_struct {
    ($($vis:vis struct $name:ident = $msg:literal;)*) => {
        $(error_struct!(@ $vis struct $name = $msg;);)*
    };
    (@ $vis:vis struct $name:ident = $msg:literal;) => {
        const _: &str = $msg;
        $vis struct $name;
        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                $msg.fmt(f)
            }
        }

        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Display::fmt(self, f)
            }
        }

        impl std::error::Error for $name { }
    };
}

error_struct! {
    pub struct CodeParseError = "could not parse code";
    pub struct SeverityParseError = "unknown severity";
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Info,
    Usage,
    Warning,
    Error,
    Fatal,
}

impl FromStr for Severity {
    type Err = SeverityParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let res = match s {
            "INFO" => Severity::Info,
            "USAGE" => Severity::Usage,
            "WARNING" => Severity::Warning,
            "ERROR" => Severity::Error,
            "FATAL" => Severity::Fatal,
            _ => return Err(SeverityParseError)
        };
        Ok(res)
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Info => "INFO",
            Severity::Usage => "USAGE",
            Severity::Warning => "WARNING",
            Severity::Error => "ERROR",
            Severity::Fatal => "FATAL",
        }.fmt(f)
    }
}


pub struct Code {
    cat: [u8; 3],
    num: u8,
}

impl FromStr for Code {
    type Err = CodeParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 7 {
            return Err(CodeParseError)
        }
        if s.as_bytes()[3] != b'-' {
            return Err(CodeParseError)
        }

        let num: u8 = s[4..].parse().map_err(|_| CodeParseError)?;
        let cat: [u8; 3] = std::array::from_fn(|i| s.as_bytes()[i]);
        Ok(Code {
            cat,
            num,
        })
    }
}

impl std::fmt::Display for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{cat}-{num}", cat = std::str::from_utf8(&self.cat).unwrap(), num = self.num)
    }
}

impl std::fmt::Debug for Code {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

#[derive(Debug)]
pub struct Message {
    pub code: Code,
    pub sev: Severity,
    pub msg: Box<str>,
}

impl std::fmt::Display for Message {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let Message { code, sev, msg } = self;
        write!(f, "{sev}({code}): {msg}")
    }
}

#[derive(Debug)]
pub struct EpubcheckResult {
    pub most_severe: Option<Severity>,
    pub msgs: Vec<Message>,
}

impl EpubcheckResult {
    pub fn is_error(&self) -> bool {
        self.most_severe.map_or(false, |sev| sev >= Severity::Error)
    }

    pub fn as_result(&self, max_sev: Severity) -> anyhow::Result<()> {
        use std::fmt::Write;
        if self.most_severe.map_or(true, |sev| sev > max_sev) {
            return Ok(())
        }

        let mut res = String::from("epubcheck errors:");
        res.reserve(self.msgs.len() * 20);
        for msg in &self.msgs {
            write!(res, "\n        {msg}").unwrap();
        }
        Err(anyhow!(res))
    }
}

pub fn epubcheck(path: impl AsRef<Path>) -> std::io::Result<EpubcheckResult> {
    // take lock since epubcheck uses all cores, so it doesn't make sense to try and compete
    let lock = LOCK.lock().unwrap();
    let res = std::process::Command::new("epubcheck").arg(path.as_ref()).output()?;
    drop(lock);
    let err = std::str::from_utf8(&res.stderr).unwrap();
    let mut msgs = Vec::new();
    let mut most_severe = None;
    for line in err.lines() {
        let (sevcode, msg) = line.split_once(": ").unwrap();
        let (sev, code) = sevcode.split_once('(').unwrap();
        let code = code.trim_end_matches(')');
        let code: Code = code.parse().unwrap();
        let sev: Severity = sev.parse().unwrap();
        most_severe = Some(most_severe.unwrap_or(Severity::Info).max(sev));
        msgs.push(Message { code, sev, msg: msg.into() })
    }
    Ok(EpubcheckResult { most_severe, msgs })
}
