use crate::*;
use serde_json::json;
use std::time::{Duration, Instant};

fn limits() -> Limits {
    Limits {
        timeout: Duration::from_millis(200),
        memory_limit_bytes: 16 * 1024 * 1024,
        stack_size_bytes: 256 * 1024,
        max_output_bytes: 16 * 1024,
    }
}

#[test]
fn strict_json_rejects_loss_at_every_depth() {
    for expression in [
        "undefined",
        "NaN",
        "Infinity",
        "-Infinity",
        "()=>1",
        "Symbol('x')",
        "1n",
        "Promise.resolve(1)",
        "new Date()",
        "new Map()",
        "new (class X {})()",
        "[,1]",
        "[undefined]",
        "(()=>{const x={};x.x=x;return x})()",
        "({bad:undefined,toJSON(){return {ok:true}}})",
        "({get bad(){throw Error('not invoked')}})",
        "new Proxy({}, {})",
        "Object.setPrototypeOf(new Date(),Object.prototype)",
        "Object.setPrototypeOf(new Map(),Object.prototype)",
        "Object.defineProperty({},'hidden',{value:undefined})",
    ] {
        let source = format!("function main() {{ return {{nested:[{{value:{expression}}}]}}; }}");
        assert_eq!(
            run_script(&source, &json!({}), limits())
                .unwrap_err()
                .code(),
            "invalid_output",
            "{expression}"
        );
    }
    for root in ["null", "[]", "1", "Promise.resolve({})"] {
        let source = format!("function main() {{ return {root}; }}");
        assert_eq!(
            run_script(&source, &json!({}), limits())
                .unwrap_err()
                .code(),
            "invalid_output"
        );
    }
}

#[test]
fn plain_json_shared_references_and_input_are_preserved() {
    let result = run_script(
        r#"function main(input) {
        const x={n:3}; return {input, a:x, b:x, dense:[null,true,"é",1.5], integral:2.0, zero:-0.0, empty:{}};
    }"#,
        &json!({"__proto__":{"data":1}, "text":"</script>"}),
        limits(),
    )
    .unwrap();
    assert_eq!(
        result.output_payload,
        json!({"input":{"__proto__":{"data":1},"text":"</script>"},
        "a":{"n":3},"b":{"n":3},"dense":[null,true,"é",1.5],"integral":2,"zero":0,"empty":{}})
    );
}

#[test]
fn trusted_intrinsics_cannot_hide_invalid_values() {
    for sabotage in [
        "JSON.stringify=()=> '{}'; Object.keys=()=>[];",
        "Object.getOwnPropertyDescriptor=()=>({value:1,enumerable:true}); Reflect.ownKeys=()=>[];",
        "Object.prototype.toJSON=()=>({}); Array.prototype.toJSON=()=>[];",
        "Object.prototype.get=()=>1; Object.prototype.value=1;",
    ] {
        let source = format!("{sabotage} function main() {{ return {{bad:undefined}}; }}");
        assert_eq!(
            run_script(&source, &json!({}), limits())
                .unwrap_err()
                .code(),
            "invalid_output"
        );
    }
    let result = run_script(
        "JSON.parse=()=>({evil:true}); function main(x){return x}",
        &json!({"expected":true}),
        limits(),
    )
    .unwrap();
    assert_eq!(result.output_payload, json!({"expected":true}));
}

#[test]
fn invocation_state_and_console_are_isolated() {
    let first = run_script("globalThis.leak=1; Object.prototype.leak=1; function main(){console.log('hi', {n:1});return {ok:true}}",
        &json!({}), limits()).unwrap();
    assert_eq!(first.console_logs[0].message, "hi {\"n\":1}");
    let second = run_script(
        "function main(){return {clean:typeof leak==='undefined'}}",
        &json!({}),
        limits(),
    )
    .unwrap();
    assert_eq!(second.output_payload, json!({"clean":true}));
    assert!(second.console_logs.is_empty());
}

#[test]
fn failure_codes_logs_and_module_deny_are_stable() {
    for (source, code) in [
        ("function main( {", "syntax_error"),
        ("const x=1", "main_missing"),
        (
            "function main(){console.error('before');throw Error('secret')}",
            "runtime_error",
        ),
        (
            "import x from 'x'; function main(){return {}}",
            "module_import_denied",
        ),
        (
            "function main(){import('x');return {}}",
            "module_import_denied",
        ),
        (
            "function main(){return {text:`${import('x')}`}}",
            "module_import_denied",
        ),
    ] {
        let error = run_script(source, &json!({}), limits()).unwrap_err();
        assert_eq!(error.code(), code, "{source}");
        assert!(!error.to_string().contains("secret"));
        if code == "runtime_error" {
            assert_eq!(error.console_logs[0].message, "before");
        }
    }
    run_script(
        "function main(){return {text:'import(x)',template:`export foo`}}",
        &json!({}),
        limits(),
    )
    .unwrap();
}

#[test]
fn output_and_console_have_a_combined_bound() {
    let small = Limits {
        max_output_bytes: 128,
        ..limits()
    };
    for source in [
        "function main(){return {s:'x'.repeat(256)}}",
        "function main(){for(let i=0;i<100;i++)console.log('abcdefgh');return {}}",
    ] {
        assert_eq!(
            run_script(source, &json!({}), small).unwrap_err().code(),
            "output_limit_exceeded"
        );
    }
}

#[test]
fn interrupt_recovery_and_started_cancel_are_joined() {
    assert_eq!(
        run_script("function main(){while(true){}}", &json!({}), limits())
            .unwrap_err()
            .code(),
        "timeout"
    );
    let stop = CancellationToken::default();
    let signal = stop.clone();
    let worker = std::thread::spawn(move || {
        run_script_cancellable(
            "function main(){while(true){}}",
            &json!({}),
            Limits {
                timeout: Duration::from_secs(30),
                ..limits()
            },
            &[],
            stop,
        )
    });
    let deadline = Instant::now() + Duration::from_secs(2);
    while !signal.has_started() {
        assert!(Instant::now() < deadline);
        std::thread::yield_now();
    }
    signal.cancel();
    assert_eq!(worker.join().unwrap().unwrap_err().code(), "cancelled");
    assert!(
        signal.has_exited(),
        "Runtime dropped before the carrier is joined"
    );
    run_script("function main(){return {ok:true}}", &json!({}), limits()).unwrap();
}

#[test]
fn pre_start_cancel_never_enters_javascript() {
    let stop = CancellationToken::default();
    stop.cancel();
    assert_eq!(
        run_script_cancellable(
            "function main(){while(true){}}",
            &json!({}),
            limits(),
            &[],
            stop.clone()
        )
        .unwrap_err()
        .code(),
        "cancelled"
    );
    assert!(!stop.has_started());
    assert!(stop.has_exited());
}

#[test]
fn memory_stack_and_invalid_limits_are_bounded_and_recoverable() {
    for source in [
        "function main(){const x=[];while(true)x.push('x'.repeat(10000))}",
        "function f(){return f()} function main(){return f()}",
    ] {
        assert!(run_script(source, &json!({}), limits()).is_err());
    }
    assert_eq!(
        run_script(
            "function main(){return {}}",
            &json!({}),
            Limits {
                max_output_bytes: 0,
                ..limits()
            }
        )
        .unwrap_err()
        .code(),
        "invalid_limits"
    );
    run_script(
        "function main(){return {}}",
        &json!({}),
        Limits {
            memory_limit_bytes: 0,
            stack_size_bytes: 0,
            ..limits()
        },
    )
    .unwrap();
}

#[test]
fn legacy_codec_preserves_omission_conversion_to_json_and_errors() {
    let output = run_legacy_script_cancellable(
        r#"function main(){
        return {omit:undefined,nested:{omit:undefined,n:NaN,p:Infinity,m:-Infinity},
          values:[undefined,,NaN,Infinity],date:new Date('2020-01-01T00:00:00Z'),
          custom:{toJSON(){return {v:7}}}};
    }"#,
        &json!({}),
        limits(),
        &[],
        CancellationToken::default(),
    )
    .unwrap();
    assert_eq!(
        output.output_payload,
        json!({"nested":{"n":null,"p":null,"m":null},
      "values":[null,null,null,null],"date":"2020-01-01T00:00:00.000Z","custom":{"v":7}})
    );
    for expression in [
        "1n",
        "(()=>{const x={};x.x=x;return x})()",
        "({toJSON(){throw Error('secret')}})",
    ] {
        let source = format!("function main(){{return {{value:{expression}}}}}");
        let error = run_legacy_script_cancellable(
            &source,
            &json!({}),
            limits(),
            &[],
            CancellationToken::default(),
        )
        .unwrap_err();
        assert_eq!(error.to_string(), "runtime_error: code execution failed");
    }
}
