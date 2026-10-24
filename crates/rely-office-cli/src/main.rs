// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at https://mozilla.org/MPL/2.0/.
//
// Copyright (c) 2026 RelyBytes Consulting <https://relybytes.com>

//! `rely-office <command> <file>`: the library from a terminal, for trying documents by hand.

use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;

use rely_office_core::{Limits, detect_format, extract_text, read_workbook};

const USAGE: &str = "usage: rely-office <detect|text|sheets> <file>

  detect   print the format read from the bytes
  text     print the text a search index would store
  sheets   print every sheet as tab-separated rows";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).collect();
    let [command, path] = arguments.as_slice() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };

    match run(command, Path::new(path)) {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("rely-office: {message}");
            ExitCode::FAILURE
        }
    }
}

fn run(command: &str, path: &Path) -> Result<String, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("{}: {error}", path.display()))?;
    let file_name = path.file_name().and_then(|name| name.to_str());
    let limits = Limits::default();

    match command {
        "detect" => Ok(format!("{:?}\n", detect_format(&bytes, file_name))),
        "text" => extract_text(&bytes, file_name, &limits).map_err(|error| error.to_string()),
        "sheets" => {
            let workbook =
                read_workbook(&bytes, file_name, &limits).map_err(|error| error.to_string())?;
            let mut output = String::new();
            for sheet in workbook.sheets {
                let _ = writeln!(
                    output,
                    "# {} ({} x {})",
                    sheet.name, sheet.total_rows, sheet.total_columns
                );
                for row in sheet.rows {
                    output.push_str(&row.join("\t"));
                    output.push('\n');
                }
            }
            Ok(output)
        }
        other => Err(format!("unknown command {other}\n{USAGE}")),
    }
}
