//! Parses the vendor `battlegroup status` text into structured fields.
//!
//! Wrapper output looks like:
//!
//! ```text
//! Battlegroup: my-bg
//! Battlegroup Info
//! Status     Database   Gateway    Director   Uptime
//! ---------- ---------- ---------- ---------- --------
//! Running    Running    Running    Running    1h2m
//!
//! Game Servers
//! Map             Phase                  Ready  Players  Age
//! --------------- ---------------------- ------ -------- ------
//! Survival_1      Running                True   3        1h
//! DeepDesert_1    Stopped                False  0        1h
//! ```
//!
//! Lines are whitespace-separated; column count is the contract, not column
//! widths.

use crate::orchestration::{BattlegroupState, ServerStatRow};

const INFO_HEADER: &str = "Battlegroup Info";
const SERVERS_HEADER: &str = "Game Servers";

/// Parses the vendor wrapper's `status` stdout into a [`BattlegroupState`].
///
/// Returns `None` when neither an info row nor a recognizable layout is
/// found, so callers can distinguish a parse failure from an empty result.
pub fn parse_wrapper_status(text: &str) -> Option<BattlegroupState> {
    let lines: Vec<&str> = text.lines().collect();
    let info_idx = lines.iter().position(|line| line.trim() == INFO_HEADER)?;
    let info_row = data_row_after_header(&lines, info_idx)?;
    let info_fields = whitespace_fields(info_row);
    if info_fields.len() < 4 {
        return None;
    }
    let phase = info_fields[0].clone();
    let database_phase = info_fields[1].clone();
    let gateway_phase = info_fields[2].clone();
    let director_phase = info_fields[3].clone();
    let uptime = info_fields.get(4).cloned().unwrap_or_default();

    let mut server_stats = Vec::new();
    if let Some(servers_idx) = lines.iter().position(|line| line.trim() == SERVERS_HEADER) {
        for row in data_rows_after_header(&lines, servers_idx) {
            let fields = whitespace_fields(row);
            if fields.len() < 5 {
                continue;
            }
            server_stats.push(ServerStatRow {
                map: fields[0].clone(),
                phase: fields[1].clone(),
                ready: fields[2].clone(),
                players: fields[3].clone(),
                age: fields[4].clone(),
            });
        }
    }

    Some(BattlegroupState {
        stop: false,
        phase,
        database_phase,
        server_group_phase: gateway_phase,
        director_phase,
        uptime,
        server_stats,
    })
}

fn data_row_after_header<'a>(lines: &'a [&'a str], header_idx: usize) -> Option<&'a str> {
    data_rows_after_header(lines, header_idx).into_iter().next()
}

fn data_rows_after_header<'a>(lines: &'a [&'a str], header_idx: usize) -> Vec<&'a str> {
    let mut rows = Vec::new();
    let mut started = false;
    for line in lines.iter().skip(header_idx + 1) {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if started {
                break;
            }
            continue;
        }
        if is_divider(trimmed) {
            started = true;
            continue;
        }
        if !started {
            // First non-empty line after the section header is the column
            // header; skip it without yet collecting data.
            started = false;
            continue;
        }
        rows.push(*line);
    }
    rows
}

fn is_divider(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c == '-' || c == '=' || c.is_whitespace())
}

fn whitespace_fields(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_string).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
Battlegroup: my-bg
Battlegroup Info
Status     Database   Gateway    Director   Uptime
---------- ---------- ---------- ---------- --------
Running    Running    Running    Running    1h2m

Game Servers
Map             Phase                  Ready  Players  Age
--------------- ---------------------- ------ -------- ------
Survival_1      Running                True   3        1h
DeepDesert_1    Stopped                False  0        1h
";

    #[test]
    fn parses_info_row_into_battlegroup_state() {
        let state = parse_wrapper_status(SAMPLE).expect("parse");
        assert!(!state.stop);
        assert_eq!(state.phase, "Running");
        assert_eq!(state.database_phase, "Running");
        assert_eq!(state.server_group_phase, "Running");
        assert_eq!(state.director_phase, "Running");
        assert_eq!(state.uptime, "1h2m");
    }

    #[test]
    fn parses_server_stats_rows() {
        let state = parse_wrapper_status(SAMPLE).expect("parse");
        assert_eq!(state.server_stats.len(), 2);
        assert_eq!(state.server_stats[0].map, "Survival_1");
        assert_eq!(state.server_stats[0].phase, "Running");
        assert_eq!(state.server_stats[0].ready, "True");
        assert_eq!(state.server_stats[0].players, "3");
        assert_eq!(state.server_stats[1].map, "DeepDesert_1");
        assert_eq!(state.server_stats[1].phase, "Stopped");
    }

    #[test]
    fn handles_missing_uptime_gracefully() {
        let text = "\
Battlegroup: x
Battlegroup Info
Status     Database   Gateway    Director   Uptime
---------- ---------- ---------- ---------- --------
Stopped    Stopped    Stopped    Stopped
";
        let state = parse_wrapper_status(text).expect("parse");
        assert_eq!(state.phase, "Stopped");
        assert_eq!(state.uptime, "");
        assert!(state.server_stats.is_empty());
    }

    #[test]
    fn returns_none_when_info_section_missing() {
        assert!(parse_wrapper_status("nothing here").is_none());
    }
}
