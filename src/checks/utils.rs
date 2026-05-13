use std::{
    borrow::Cow,
    fmt::Display,
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::Path,
    time::Duration,
};

use serde::{Deserialize, Deserializer, de::Error};

use crate::{exec::CommandRunner, opnsense as opnsense_data};

pub fn read_opnsense_config() -> Option<opnsense_data::config_xml::OpnsenseConfig> {
    opnsense_data::config_xml::read_config(Path::new("/conf/config.xml"))
}

pub fn pidof(runner: &CommandRunner, process_name: &str) -> Option<i64> {
    let data = runner.run("ps", ["ax", "-c", "-o", "command,pid"]).ok()?;
    data.lines().find_map(|line| {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        (parts.len() == 2 && parts[0] == process_name)
            .then(|| parts[1].parse::<i64>().ok())
            .flatten()
    })
}

pub fn unix_socket_command(path: &str, command: &[u8]) -> Option<String> {
    let mut stream = UnixStream::connect(path).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    stream
        .set_write_timeout(Some(Duration::from_secs(2)))
        .ok()?;
    stream.write_all(command).ok()?;
    let mut data = String::new();
    stream.read_to_string(&mut data).ok()?;
    Some(data)
}

pub fn unix_socket_http(path: &str, request: &[u8]) -> Option<String> {
    unix_socket_command(path, request)
}

pub fn split_http_response(response: &str) -> (Option<u16>, &str) {
    let status = response
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|code| code.parse::<u16>().ok());
    let body = response
        .split_once("\r\n\r\n")
        .or_else(|| response.split_once("\n\n"))
        .map(|(_, body)| body)
        .unwrap_or(response);
    (status, body)
}

pub struct Percentage(pub f64);

impl<'de> Deserialize<'de> for Percentage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let string: Cow<str> = Cow::deserialize(deserializer)?;
        let string = string.trim();
        let pct = if let Some(numeric) = string.strip_suffix('%').map(|s| s.trim()) {
            let v: f64 = numeric.parse().map_err(Error::custom)?;
            v / 100.0
        } else {
            let v: f64 = string.parse().map_err(Error::custom)?;
            if v >= 1.0 { v / 100.0 } else { v }
        };
        Ok(Percentage(pct))
    }
}

impl Display for Percentage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}%", self.0)
    }
}
