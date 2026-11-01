use crate::{codec::StrictCodec, CancellationToken, ConsoleLogEntry, ErrorKind, ExecutionError};
use rquickjs::{function::Rest, Ctx, Function, Object, Value as JsValue};
use serde_json::json;
use std::{cell::RefCell, rc::Rc, time::Instant};

#[derive(Default)]
struct State {
    logs: Vec<ConsoleLogEntry>,
    bytes: usize,
    overflow: bool,
}
pub(crate) struct ConsoleCapture<'js> {
    state: Rc<RefCell<State>>,
    max: usize,
    legacy: bool,
    codec: Rc<RefCell<Option<StrictCodec<'js>>>>,
}
impl<'js> ConsoleCapture<'js> {
    pub fn install(
        ctx: &Ctx<'js>,
        max: usize,
        legacy: bool,
        codec: StrictCodec<'js>,
        cancel: CancellationToken,
        deadline: Instant,
    ) -> Result<Self, ExecutionError> {
        let state = Rc::new(RefCell::new(State::default()));
        let codec = Rc::new(RefCell::new(Some(codec)));
        // Construct the cleanup owner BEFORE any callback is registered: even an
        // allocation failure during install must release captured JS references.
        let capture = Self {
            state: state.clone(),
            max,
            legacy,
            codec: codec.clone(),
        };
        if legacy {
            install_legacy_console(ctx).map_err(|_| ErrorKind::RuntimeError)?;
        } else {
            let console = Object::new(ctx.clone()).map_err(|_| ErrorKind::RuntimeError)?;
            for (name, level) in [
                ("log", "info"),
                ("info", "info"),
                ("warn", "warn"),
                ("error", "error"),
            ] {
                let state = state.clone();
                let codec = codec.clone();
                let cancel = cancel.clone();
                let callback = Function::new(ctx.clone(), move |args: Rest<JsValue<'js>>| {
                    if state.borrow().overflow {
                        return;
                    }
                    let mut normalized = Vec::new();
                    let mut messages = Vec::new();
                    // Per-entry bounded scratch; no guest code runs to serialize console.
                    for arg in args.0 {
                        let value = if arg.is_undefined() {
                            json!({"type":"undefined"})
                        } else if let Some(n) = arg.as_number().filter(|n| !n.is_finite()) {
                            json!(if n.is_nan() {
                                "NaN"
                            } else if n > 0.0 {
                                "Infinity"
                            } else {
                                "-Infinity"
                            })
                        } else {
                            let slot = codec.borrow();
                            let Some(codec) = slot.as_ref() else {
                                return;
                            };
                            match codec.encode(arg, max, &cancel, deadline) {
                                Ok(v) => v,
                                Err(e) if e.code() == "output_limit_exceeded" => {
                                    state.borrow_mut().overflow = true;
                                    return;
                                }
                                Err(_) => json!("[Unserializable]"),
                            }
                        };
                        let message = if value == json!({"type":"undefined"}) {
                            "undefined".to_string()
                        } else if let Some(s) = value.as_str() {
                            s.to_string()
                        } else {
                            value.to_string()
                        };
                        messages.push(message);
                        normalized.push(value);
                        if messages.iter().map(String::len).sum::<usize>() > max {
                            state.borrow_mut().overflow = true;
                            return;
                        }
                    }
                    let entry = ConsoleLogEntry {
                        level: level.to_string(),
                        message: messages.join(" "),
                        args: normalized,
                    };
                    let bytes =
                        serde_json::to_vec(&entry).map_or(max.saturating_add(1), |v| v.len());
                    let mut state = state.borrow_mut();
                    let framing = if state.logs.is_empty() { 2 } else { 1 };
                    if state.bytes.saturating_add(bytes).saturating_add(framing) > max {
                        state.overflow = true;
                        return;
                    }
                    state.bytes += bytes + framing;
                    state.logs.push(entry);
                })
                .map_err(|_| ErrorKind::RuntimeError)?;
                console
                    .set(name, callback)
                    .map_err(|_| ErrorKind::RuntimeError)?;
            }
            ctx.globals()
                .set("console", console)
                .map_err(|_| ErrorKind::RuntimeError)?;
        }
        Ok(capture)
    }
    pub fn read(&self, ctx: &Ctx<'_>) -> (Vec<ConsoleLogEntry>, usize, bool) {
        if self.legacy {
            let mut logs = read_legacy_logs(ctx);
            let mut bytes = 0usize;
            let mut overflow = false;
            logs.retain(|log| {
                let n = serde_json::to_vec(log).map_or(self.max.saturating_add(1), |v| v.len());
                let framing = if bytes == 0 { 2 } else { 1 };
                if bytes.saturating_add(n).saturating_add(framing) > self.max {
                    overflow = true;
                    false
                } else {
                    bytes += n + framing;
                    true
                }
            });
            (logs, bytes, overflow)
        } else {
            let s = self.state.borrow();
            (s.logs.clone(), s.bytes, s.overflow)
        }
    }
}

impl Drop for ConsoleCapture<'_> {
    fn drop(&mut self) {
        // rquickjs RustFunction does not trace closure captures. Break references
        // to JS intrinsics before Context/Runtime destruction, including failures.
        self.codec.borrow_mut().take();
    }
}

fn install_legacy_console(ctx: &Ctx<'_>) -> rquickjs::Result<()> {
    ctx.eval::<(), _>(
        r#"
(function () {
  const logs = [];

  function normalizeConsoleArg(value, seen) {
    if (value === undefined) {
      return { type: "undefined" };
    }
    if (value === null || typeof value === "string" || typeof value === "boolean") {
      return value;
    }
    if (typeof value === "number") {
      return Number.isFinite(value) ? value : String(value);
    }
    if (typeof value === "bigint" || typeof value === "symbol") {
      return String(value);
    }
    if (typeof value === "function") {
      return value.name ? "[Function " + value.name + "]" : "[Function]";
    }
    if (seen.indexOf(value) >= 0) {
      return "[Circular]";
    }

    const nextSeen = seen.concat([value]);
    if (Array.isArray(value)) {
      return value.map(function (item) {
        try {
          return normalizeConsoleArg(item, nextSeen);
        } catch (_error) {
          return "[Unserializable]";
        }
      });
    }

    if (value instanceof Error) {
      return { name: value.name, message: value.message };
    }

    const output = {};
    Object.keys(value).forEach(function (key) {
      try {
        output[key] = normalizeConsoleArg(value[key], nextSeen);
      } catch (_error) {
        output[key] = "[Unserializable]";
      }
    });
    return output;
  }

  function formatConsoleArg(value) {
    if (typeof value === "string") {
      return value;
    }
    if (value && typeof value === "object" && value.type === "undefined") {
      return "undefined";
    }
    if (value && typeof value === "object") {
      try {
        const json = JSON.stringify(value);
        return json === undefined ? String(value) : json;
      } catch (_error) {
        return "[Unserializable]";
      }
    }
    return String(value);
  }

  function captureConsole(level, args) {
    const normalizedArgs = Array.prototype.map.call(args, function (arg) {
      return normalizeConsoleArg(arg, []);
    });
    logs.push({
      level: level,
      message: normalizedArgs.map(formatConsoleArg).join(" "),
      args: normalizedArgs
    });
  }

  Object.defineProperty(globalThis, "__oneflowbase_console_logs", {
    value: logs,
    configurable: false,
    enumerable: false,
    writable: false
  });
  Object.defineProperty(globalThis, "console", {
    value: Object.freeze({
      log: function () { captureConsole("info", arguments); },
      info: function () { captureConsole("info", arguments); },
      warn: function () { captureConsole("warn", arguments); },
      error: function () { captureConsole("error", arguments); }
    }),
    configurable: true,
    enumerable: false,
    writable: true
  });
})();
"#,
    )?;
    Ok(())
}

fn read_legacy_logs(ctx: &Ctx<'_>) -> Vec<ConsoleLogEntry> {
    let Ok(logs_json) =
        ctx.eval::<String, _>(r#"JSON.stringify(globalThis.__oneflowbase_console_logs || [])"#)
    else {
        return Vec::new();
    };
    serde_json::from_str(&logs_json).unwrap_or_default()
}
