use std::fmt::{self, Display, Formatter};

#[derive(Debug, Default)]
pub struct LocalSection {
    rows: Vec<LocalRow>,
}

impl LocalSection {
    pub fn new() -> Self {
        Self { rows: Vec::new() }
    }

    pub fn empty() -> Self {
        Self::new()
    }

    pub fn row(
        &mut self,
        state: LocalState,
        service: impl Display,
        summary: impl Display,
    ) -> &mut LocalRow {
        self.rows.push_mut(LocalRow {
            state,
            service: service.to_string(),
            metrics: Vec::new(),
            summary: summary.to_string(),
        })
    }

    pub fn inject(&mut self, key: impl Display, value: impl Display) -> &mut Self {
        if let Some(row) = self.rows.first_mut() {
            row.with_metric(key, value);
        }
        self
    }
}

impl Display for LocalSection {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        if self.rows.is_empty() {
            return Ok(());
        }
        writeln!(f, "<<<local:sep(0)>>>")?;
        let rows: Vec<String> = self.rows.iter().map(|r| r.to_string()).collect();
        let rows = rows.join("\n");
        write!(f, "{rows}")
    }
}

#[derive(Debug)]
pub struct LocalRow {
    state: LocalState,
    service: String,
    metrics: Vec<LocalMetric>,
    summary: String,
}

impl LocalRow {
    pub fn with_metric(&mut self, key: impl Display, value: impl Display) -> &mut Self {
        self.metrics.push(LocalMetric {
            key: key.to_string(),
            value: value.to_string(),
        });
        self
    }

    fn escape_service_name(&self) -> String {
        self.service.replace('"', "'").replace('\n', " ")
    }

    fn render_metrics(&self) -> String {
        if self.metrics.is_empty() {
            return "-".to_owned();
        }

        self.metrics
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("|")
    }
}

impl Display for LocalRow {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} \"{}\" {} {}",
            self.state.as_str(),
            self.escape_service_name(),
            self.render_metrics(),
            self.summary.replace('\n', "\\n")
        )
    }
}

#[derive(Debug)]
struct LocalMetric {
    key: String,
    value: String,
}

impl Display for LocalMetric {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value)
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

pub fn collect_sections<I>(sections: I) -> String
where
    I: IntoIterator<Item = LocalSection>,
{
    let output = sections
        .into_iter()
        .map(|section| section.to_string())
        .collect::<Vec<_>>()
        .join("\n");

    if output.is_empty() {
        String::new()
    } else {
        format!("{output}\n")
    }
}
