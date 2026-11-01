use crate::{check_stop, CancellationToken, ErrorKind, ExecutionError};
use rquickjs::{
    object::{Filter, ObjectKeysIter},
    Atom, Ctx, Function, Object, Value as JsValue,
};
use serde_json::Value;
use std::{
    io::{self, Write},
    time::Instant,
};

/// Host traversal never calls toJSON, enumerable getters, or guest-replaced helpers.
#[derive(Clone)]
pub(crate) struct StrictCodec<'js> {
    descriptor: Function<'js>,
    object_prototype: Object<'js>,
    array_prototype: Object<'js>,
    object_class: rquickjs::qjs::JSClassID,
    array_class: rquickjs::qjs::JSClassID,
}
impl<'js> StrictCodec<'js> {
    pub fn capture(ctx: &Ctx<'js>) -> rquickjs::Result<Self> {
        let plain: Object = ctx.eval("({})")?;
        let array: Object = ctx.eval("[]")?;
        Ok(Self {
            object_class: class_id(plain.as_value()),
            array_class: class_id(array.as_value()),
            descriptor: ctx.eval("Object.getOwnPropertyDescriptor")?,
            object_prototype: ctx.eval("Object.prototype")?,
            array_prototype: ctx.eval("Array.prototype")?,
        })
    }
    fn data(&self, object: &Object<'js>, key: &str) -> Result<JsValue<'js>, ExecutionError> {
        let descriptor: JsValue = self
            .descriptor
            .call((object.clone(), key))
            .map_err(|_| ErrorKind::InvalidOutput)?;
        let descriptor = descriptor.as_object().ok_or(ErrorKind::InvalidOutput)?;
        // Accessor descriptors have no own "value". Check using the captured intrinsic;
        // looking up descriptor.value alone would trust a polluted Object.prototype.
        let own_value: JsValue = self
            .descriptor
            .call((descriptor.clone(), "value"))
            .map_err(|_| ErrorKind::InvalidOutput)?;
        if own_value.is_undefined() {
            return Err(ErrorKind::InvalidOutput.into());
        }
        descriptor
            .get("value")
            .map_err(|_| ErrorKind::InvalidOutput.into())
    }
    pub fn encode_root(
        &self,
        value: JsValue<'js>,
        max: usize,
        stop: &CancellationToken,
        deadline: Instant,
    ) -> Result<Value, ExecutionError> {
        if !value.is_object() || value.is_array() {
            return Err(ErrorKind::InvalidOutput.into());
        }
        self.encode(value, max, stop, deadline)
    }
    pub fn encode(
        &self,
        value: JsValue<'js>,
        max: usize,
        stop: &CancellationToken,
        deadline: Instant,
    ) -> Result<Value, ExecutionError> {
        let mut out = BoundedBytes {
            bytes: Vec::new(),
            max,
        };
        self.walk(value, &mut out, stop, deadline)?;
        serde_json::from_slice(&out.bytes).map_err(|_| ErrorKind::InvalidOutput.into())
    }
    fn walk(
        &self,
        value: JsValue<'js>,
        out: &mut BoundedBytes,
        stop: &CancellationToken,
        deadline: Instant,
    ) -> Result<(), ExecutionError> {
        let mut ancestors = Vec::new();
        let mut frames = vec![WalkFrame::Value(value)];
        // Each container keeps only its next-child continuation on the heap.
        // Exit removes it from the current ancestor path, so a shared DAG node
        // is legal after its earlier branch finishes. No Rust traversal recursion.
        while let Some(frame) = frames.pop() {
            check_stop(stop, deadline)?;
            match frame {
                WalkFrame::Value(value) => {
                    if value.is_null() {
                        out.raw(b"null")?;
                        continue;
                    }
                    if let Some(b) = value.as_bool() {
                        out.json(&b)?;
                        continue;
                    }
                    if let Some(n) = value.as_number() {
                        if !n.is_finite() {
                            return Err(ErrorKind::InvalidOutput.into());
                        }
                        if let Some(n) = value.as_int() {
                            out.json(&n)?;
                        } else if n.fract() == 0.0 && n.abs() <= ((1_u64 << 53) - 1) as f64 {
                            // Native JSON.parse may store integral input as Float64. Preserve
                            // JSON's single number domain (including -0 -> 0), without
                            // calling guest Number/toJSON or rounding a fractional value.
                            out.json(&(n as i64))?;
                        } else {
                            out.json(&n)?;
                        }
                        continue;
                    }
                    if let Some(text) = value.as_string() {
                        let text = text.to_string().map_err(|_| ErrorKind::InvalidOutput)?;
                        out.json(&text)?;
                        continue;
                    }
                    if !value.is_object()
                        || value.is_function()
                        || value.is_promise()
                        || value.is_proxy()
                        || ancestors.contains(&value)
                    {
                        return Err(ErrorKind::InvalidOutput.into());
                    }
                    let object = value.as_object().ok_or(ErrorKind::InvalidOutput)?;
                    let array = value.as_array();
                    if class_id(&value)
                        != if array.is_some() {
                            self.array_class
                        } else {
                            self.object_class
                        }
                    {
                        return Err(ErrorKind::InvalidOutput.into());
                    }
                    let prototype = object.get_prototype();
                    if array.is_some() {
                        if prototype.as_ref() != Some(&self.array_prototype) {
                            return Err(ErrorKind::InvalidOutput.into());
                        }
                    } else if prototype.is_some()
                        && prototype.as_ref() != Some(&self.object_prototype)
                    {
                        return Err(ErrorKind::InvalidOutput.into());
                    }
                    ancestors.push(value.clone());
                    if let Some(array) = array {
                        let len = array.len();
                        if len > out.max {
                            return Err(ErrorKind::OutputLimit.into());
                        }
                        // Reject non-index data (including symbols), rather than silently dropping it.
                        for key in object.own_keys::<Atom>(Filter::new().string().symbol()) {
                            check_stop(stop, deadline)?;
                            let key = key
                                .map_err(|_| ErrorKind::InvalidOutput)?
                                .to_value()
                                .map_err(|_| ErrorKind::InvalidOutput)?;
                            let key = key
                                .as_string()
                                .ok_or(ErrorKind::InvalidOutput)?
                                .to_string()
                                .map_err(|_| ErrorKind::InvalidOutput)?;
                            if key != "length"
                                && !key
                                    .parse::<usize>()
                                    .ok()
                                    .is_some_and(|i| i < len && i.to_string() == key)
                            {
                                return Err(ErrorKind::InvalidOutput.into());
                            }
                        }
                        out.raw(b"[")?;
                        frames.push(WalkFrame::Exit(b']'));
                        frames.push(WalkFrame::Array {
                            object: object.clone(),
                            index: 0,
                            len,
                        });
                    } else {
                        out.raw(b"{")?;
                        frames.push(WalkFrame::Exit(b'}'));
                        frames.push(WalkFrame::Object {
                            object: object.clone(),
                            keys: object.own_keys::<Atom>(Filter::new().string().symbol()),
                            first: true,
                        });
                    }
                }
                WalkFrame::Array { object, index, len } => {
                    if index == len {
                        continue;
                    }
                    if index != 0 {
                        out.raw(b",")?;
                    }
                    let child = self.data(&object, &index.to_string())?;
                    frames.push(WalkFrame::Array {
                        object,
                        index: index + 1,
                        len,
                    });
                    frames.push(WalkFrame::Value(child));
                }
                WalkFrame::Object {
                    object,
                    mut keys,
                    first,
                } => {
                    let Some(key) = keys.next() else {
                        continue;
                    };
                    let key = key
                        .map_err(|_| ErrorKind::InvalidOutput)?
                        .to_value()
                        .map_err(|_| ErrorKind::InvalidOutput)?;
                    let key = key
                        .as_string()
                        .ok_or(ErrorKind::InvalidOutput)?
                        .to_string()
                        .map_err(|_| ErrorKind::InvalidOutput)?;
                    let descriptor: Object = self
                        .descriptor
                        .call((object.clone(), key.as_str()))
                        .map_err(|_| ErrorKind::InvalidOutput)?;
                    if !descriptor
                        .get::<_, bool>("enumerable")
                        .map_err(|_| ErrorKind::InvalidOutput)?
                    {
                        return Err(ErrorKind::InvalidOutput.into());
                    }
                    if !first {
                        out.raw(b",")?;
                    }
                    out.json(&key)?;
                    out.raw(b":")?;
                    let child = self.data(&object, &key)?;
                    frames.push(WalkFrame::Object {
                        object,
                        keys,
                        first: false,
                    });
                    frames.push(WalkFrame::Value(child));
                }
                WalkFrame::Exit(close) => {
                    out.raw(&[close])?;
                    ancestors.pop();
                }
            }
        }
        Ok(())
    }
}

enum WalkFrame<'js> {
    Value(JsValue<'js>),
    Array {
        object: Object<'js>,
        index: usize,
        len: usize,
    },
    Object {
        object: Object<'js>,
        keys: ObjectKeysIter<'js, Atom<'js>>,
        first: bool,
    },
    Exit(u8),
}

struct BoundedBytes {
    bytes: Vec<u8>,
    max: usize,
}
impl BoundedBytes {
    fn raw(&mut self, bytes: &[u8]) -> Result<(), ExecutionError> {
        self.write_all(bytes)
            .map_err(|_| ErrorKind::OutputLimit.into())
    }
    fn json(&mut self, value: &impl serde::Serialize) -> Result<(), ExecutionError> {
        serde_json::to_writer(self, value).map_err(|_| ErrorKind::OutputLimit.into())
    }
}
impl Write for BoundedBytes {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.max.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other("output byte limit"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn class_id(value: &JsValue<'_>) -> rquickjs::qjs::JSClassID {
    // SAFETY: caller supplies an owned object Value from this live Context. QuickJS
    // only reads its engine class ID; it invokes no guest code and retains nothing.
    unsafe { rquickjs::qjs::JS_GetClassID(value.as_raw()) }
}
