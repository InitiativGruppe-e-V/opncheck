#[derive(Debug, Default)]
pub struct LocalSection {
    lines: Vec<String>,
}

impl LocalSection {
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }

    pub fn empty() -> Self {
        Self::new()
    }

    pub fn add(&mut self, state: LocalState, service: &str, metrics: &str, summary: &str) {
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

    pub fn finalize<I>(sections: I) -> String
    where
        I: IntoIterator<Item = Self>,
    {
        let mut lines = Vec::new();

        for section in sections {
            if section.lines.is_empty() {
                continue;
            }

            lines.push("<<<local:sep(0)>>>".to_owned());
            lines.extend(section.lines);
        }

        lines.push(String::new());
        lines.join("\n")
    }
}

#[macro_export]
macro_rules! skip_check {
    () => {
        return Ok($crate::plugin::output::LocalSection::empty())
    };
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
