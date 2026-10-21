use crate::error::{CliError, CliErrorKind};
use serde::Serialize;
use serde_json::Value;
use std::io::{self, ErrorKind, Write};

pub fn json(value: &Value, compact: bool) -> Result<(), CliError> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    let result = if compact {
        serde_json::to_writer(&mut output, value)
    } else {
        serde_json::to_writer_pretty(&mut output, value)
    };
    if let Err(error) = result {
        if error.io_error_kind() == Some(ErrorKind::BrokenPipe) {
            return Ok(());
        }
        return Err(write_error(error.to_string()));
    }
    finish_line(&mut output)
}

pub fn text_lines(lines: &[String]) -> Result<(), CliError> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    for line in lines {
        if let Err(error) = output
            .write_all(line.as_bytes())
            .and_then(|_| output.write_all(b"\n"))
        {
            if error.kind() == ErrorKind::BrokenPipe {
                return Ok(());
            }
            return Err(write_error(error.to_string()));
        }
    }
    Ok(())
}

pub fn raw(bytes: &[u8]) -> Result<(), CliError> {
    let stdout = io::stdout();
    let mut output = stdout.lock();
    if let Err(error) = output.write_all(bytes) {
        if error.kind() == ErrorKind::BrokenPipe {
            return Ok(());
        }
        return Err(write_error(error.to_string()));
    }
    Ok(())
}

pub fn print_error(error: &CliError) {
    if error.json {
        let report = ErrorReport {
            error: ErrorDetail {
                code: error.kind.code(),
                message: &error.message,
                path: error
                    .path
                    .as_ref()
                    .map(|path| path.to_string_lossy().into_owned()),
                namespace: error.namespace.as_deref(),
                key: error.key.as_deref(),
            },
        };
        let stderr = io::stderr();
        let mut output = stderr.lock();
        let _ = serde_json::to_writer(&mut output, &report);
        let _ = writeln!(output);
    } else if let Some(path) = &error.path {
        eprintln!("xmeta: {}: {}", path.display(), error.message);
    } else {
        eprintln!("xmeta: {}", error.message);
    }
}

fn finish_line(output: &mut impl Write) -> Result<(), CliError> {
    if let Err(error) = output.write_all(b"\n") {
        if error.kind() == ErrorKind::BrokenPipe {
            return Ok(());
        }
        return Err(write_error(error.to_string()));
    }
    Ok(())
}

fn write_error(message: String) -> CliError {
    CliError::new(CliErrorKind::Io, format!("cannot write stdout: {message}"))
}

#[derive(Serialize)]
struct ErrorReport<'a> {
    error: ErrorDetail<'a>,
}

#[derive(Serialize)]
struct ErrorDetail<'a> {
    code: &'static str,
    message: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    namespace: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    key: Option<&'a str>,
}
