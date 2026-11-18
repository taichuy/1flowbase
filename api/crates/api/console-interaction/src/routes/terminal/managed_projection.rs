//! Fixed safe views. String/body contents and filesystem/record identifiers never leave this owner.
use super::{TerminalInput, TerminalOutput};
use crate::extension_bus::managed_projection as mp;
use serde_json::{json, Value};

fn shape(tag_field: &str, tag: &str, counts: &[&str], flags: &[&str]) -> Value {
    mp::object_schema(
        std::iter::once((tag_field, mp::tag_schema(tag)))
            .chain(counts.iter().map(|name| (*name, mp::count_schema())))
            .chain(flags.iter().map(|name| (*name, json!({"type":"boolean"})))),
    )
}

pub(super) fn input_schema() -> Value {
    mp::union_schema(vec![
        shape("action", "info", &[], &[]),
        shape("action", "files.list", &["limit", "timeout_ms"], &[]),
        shape("action", "files.stat", &["timeout_ms"], &[]),
        shape(
            "action",
            "files.read_text",
            &["offset", "max_bytes", "timeout_ms"],
            &[],
        ),
        shape(
            "action",
            "files.read_range",
            &["offset", "length", "timeout_ms"],
            &[],
        ),
        shape(
            "action",
            "files.search",
            &[
                "pattern_bytes",
                "max_scan_bytes",
                "max_matches",
                "max_depth",
                "max_line_bytes",
                "timeout_ms",
            ],
            &[],
        ),
        shape(
            "action",
            "files.read_image",
            &["max_file_bytes", "timeout_ms"],
            &[],
        ),
        shape(
            "action",
            "files.read_audio",
            &["max_file_bytes", "timeout_ms"],
            &[],
        ),
        shape(
            "action",
            "files.read_resource",
            &["max_file_bytes", "timeout_ms"],
            &["link_only"],
        ),
        shape("action", "files.import_resource", &[], &[]),
        shape(
            "action",
            "files.download_file",
            &["timeout_ms"],
            &["overwrite"],
        ),
        shape(
            "action",
            "jobs.start",
            &[
                "command_bytes",
                "timeout_ms",
                "retention_ms",
                "max_output_bytes",
            ],
            &[],
        ),
        shape("action", "jobs.get", &[], &[]),
        shape("action", "jobs.read", &["offset", "max_bytes"], &[]),
        shape("action", "jobs.cancel", &[], &[]),
    ])
}

pub(super) fn input(value: &TerminalInput) -> Value {
    match value {
        TerminalInput::Info(_) => json!({"action":"info"}),
        TerminalInput::List(v) => {
            json!({"action":"files.list","limit":v.limit,"timeout_ms":v.timeout_ms})
        }
        TerminalInput::Stat(v) => json!({"action":"files.stat","timeout_ms":v.timeout_ms}),
        TerminalInput::Text(v) => {
            json!({"action":"files.read_text","offset":v.offset,"max_bytes":v.max_bytes,"timeout_ms":v.timeout_ms})
        }
        TerminalInput::Range(v) => {
            json!({"action":"files.read_range","offset":v.offset,"length":v.length,"timeout_ms":v.timeout_ms})
        }
        TerminalInput::Search(v) => {
            json!({"action":"files.search","pattern_bytes":v.pattern.len(),"max_scan_bytes":v.max_scan_bytes,"max_matches":v.max_matches,"max_depth":v.max_depth,"max_line_bytes":v.max_line_bytes,"timeout_ms":v.timeout_ms})
        }
        TerminalInput::Image(v) => {
            json!({"action":"files.read_image","max_file_bytes":v.max_file_bytes,"timeout_ms":v.timeout_ms})
        }
        TerminalInput::Audio(v) => {
            json!({"action":"files.read_audio","max_file_bytes":v.max_file_bytes,"timeout_ms":v.timeout_ms})
        }
        TerminalInput::Resource(v) => {
            json!({"action":"files.read_resource","max_file_bytes":v.max_file_bytes,"timeout_ms":v.timeout_ms,"link_only":v.link_only})
        }
        TerminalInput::Import(_) => json!({"action":"files.import_resource"}),
        TerminalInput::Download(v) => {
            json!({"action":"files.download_file","timeout_ms":v.timeout_ms,"overwrite":v.overwrite})
        }
        TerminalInput::Start(v) => {
            json!({"action":"jobs.start","command_bytes":v.command.len(),"timeout_ms":v.timeout_ms,"retention_ms":v.retention_ms,"max_output_bytes":v.max_output_bytes})
        }
        TerminalInput::Get(_) => json!({"action":"jobs.get"}),
        TerminalInput::Read(v) => {
            json!({"action":"jobs.read","offset":v.offset,"max_bytes":v.max_bytes})
        }
        TerminalInput::Cancel(_) => json!({"action":"jobs.cancel"}),
    }
}

pub(super) fn output_schema() -> Value {
    let job = mp::object_schema(
        [
            ("kind", mp::tag_schema("job")),
            ("state", json!({"type":"string","maxLength":9,"enum":["running","completed","failed","cancelled","timed_out"]})),
        ].into_iter()
            .chain(["duration_ms", "stdout_bytes", "stderr_bytes", "discarded_bytes"].into_iter().map(|name| (name, mp::count_schema())))
            .chain(["cancel_requested", "output_complete", "cleanup_complete", "truncated"].into_iter().map(|name| (name, json!({"type":"boolean"})))),
    );
    mp::union_schema(vec![
        shape("kind", "info", &["capability_count"], &["pty"]),
        shape("kind", "list", &["entry_count"], &["eof"]),
        shape("kind", "stat", &["file_bytes"], &[]),
        shape("kind", "text", &["text_bytes"], &["eof"]),
        shape("kind", "range", &["byte_count"], &["eof"]),
        shape(
            "kind",
            "search",
            &[
                "match_count",
                "scanned_bytes",
                "skipped_long_lines",
                "skipped_binary_files",
                "skipped_special_files",
            ],
            &["truncated", "depth_limited"],
        ),
        shape("kind", "media", &["encoded_bytes", "file_bytes"], &[]),
        shape(
            "kind",
            "resource",
            &["encoded_bytes", "file_bytes"],
            &["link_only"],
        ),
        shape("kind", "import", &["record_field_count"], &[]),
        shape("kind", "download", &["byte_count"], &[]),
        job,
        shape("kind", "read", &["text_bytes"], &["eof", "encoding_lossy"]),
    ])
}

pub(super) fn output(value: &TerminalOutput) -> Value {
    // O(1) length/metadata access: do not serialize, traverse or copy potentially large content.
    match value {
        TerminalOutput::Info(v) => {
            json!({"kind":"info","capability_count":v.capabilities.len(),"pty":v.pty})
        }
        TerminalOutput::List(v) => json!({"kind":"list","entry_count":v.entries.len(),"eof":v.eof}),
        TerminalOutput::Stat(v) => json!({"kind":"stat","file_bytes":v.size}),
        TerminalOutput::Text(v) => json!({"kind":"text","text_bytes":v.text.len(),"eof":v.eof}),
        TerminalOutput::Range(v) => {
            json!({"kind":"range","byte_count":v.metadata.byte_count,"eof":v.metadata.eof})
        }
        TerminalOutput::Search(v) => {
            json!({"kind":"search","match_count":v.matches.len(),"scanned_bytes":v.scanned_bytes,"truncated":v.truncated,"skipped_long_lines":v.skipped_long_lines,"skipped_binary_files":v.skipped_binary_files,"skipped_special_files":v.skipped_special_files,"depth_limited":v.depth_limited})
        }
        TerminalOutput::Media(v) => {
            json!({"kind":"media","encoded_bytes":v.data.len(),"file_bytes":v.metadata.size})
        }
        TerminalOutput::Resource(v) => {
            json!({"kind":"resource","encoded_bytes":v.data.as_ref().map_or(0, String::len),"file_bytes":v.metadata.size,"link_only":v.data.is_none()})
        }
        TerminalOutput::Import(v) => {
            json!({"kind":"import","record_field_count":v.record.as_object().map_or(0, serde_json::Map::len)})
        }
        TerminalOutput::Download(v) => json!({"kind":"download","byte_count":v.byte_count}),
        TerminalOutput::Job(v) => {
            json!({"kind":"job","state":v.state,"duration_ms":v.duration_ms,"stdout_bytes":v.stdout_bytes,"stderr_bytes":v.stderr_bytes,"discarded_bytes":v.discarded_bytes,"cancel_requested":v.cancel_requested,"output_complete":v.output_complete,"cleanup_complete":v.cleanup_complete,"truncated":v.truncated})
        }
        TerminalOutput::Read(v) => {
            json!({"kind":"read","text_bytes":v.text.len(),"eof":v.eof,"encoding_lossy":v.encoding_lossy})
        }
    }
}
