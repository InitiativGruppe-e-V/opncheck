use std::ops::AddAssign;

#[derive(Debug, Default)]
pub struct AgentOutput {
    lines: Vec<String>,
}

impl AgentOutput {
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }

    pub fn section(&mut self, name: impl AsRef<str>) {
        self.lines.push(format!("<<<{}>>>", name.as_ref()));
    }

    pub fn section_cached(&mut self, name: impl AsRef<str>, created: i64, cachetime: u64) {
        self.lines.push(format!(
            "<<<{}:cached({created},{cachetime})>>>",
            name.as_ref()
        ));
    }

    pub fn line(&mut self, line: impl Into<String>) {
        self.lines.push(line.into());
    }

    pub fn extend<I, S>(&mut self, lines: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.lines.extend(lines.into_iter().map(Into::into));
    }

    pub fn raw_block(&mut self, block: &str) {
        self.lines.extend(block.lines().map(str::to_owned));
    }

    pub fn local(&mut self, state: LocalState, service: &str, metrics: &str, summary: &str) {
        self.lines.push(format!(
            "{} \"{}\" {} {}",
            state.as_str(),
            escape_service_name(service),
            if metrics.trim().is_empty() {
                "-"
            } else {
                metrics
            },
            summary.replace('\n', "\\n")
        ));
    }

    pub fn finish(mut self) -> String {
        self.lines.push(String::new());
        self.lines.join("\n")
    }
}

impl AddAssign for AgentOutput {
    fn add_assign(&mut self, rhs: Self) {
        self.extend(rhs.lines);
    }
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum LocalState {
    Ok,
    Warn,
    Crit,
    Unknown,
    Dynamic,
}

impl LocalState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "0",
            Self::Warn => "1",
            Self::Crit => "2",
            Self::Unknown => "3",
            Self::Dynamic => "P",
        }
    }
}

fn escape_service_name(input: &str) -> String {
    input.replace('"', "'").replace('\n', " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_check_format_is_checkmk_compatible() {
        let mut out = AgentOutput::new();
        out.section("local:sep(0)");
        out.local(LocalState::Ok, "My Service", "value=1", "all good");
        assert_eq!(
            out.finish(),
            "<<<local:sep(0)>>>
0 \"My Service\" value=1 all good
"
        );
    }
}
