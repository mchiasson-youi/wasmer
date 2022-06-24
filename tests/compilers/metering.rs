use anyhow::Result;
use wasmer_middlewares::Metering;

use std::sync::Arc;
use wasmer::wasmparser::Operator;
use wasmer::Context as WasmerContext;
use wasmer::*;

fn cost_always_one(_: &Operator) -> u64 {
    1
}

fn run_add_with_limit(mut config: crate::Config, limit: u64) -> Result<()> {
    config
        .middlewares
        .push(Arc::new(Metering::new(limit, cost_always_one)));
    let store = config.store();
    let wat = r#"(module
        (func (export "add") (param i32 i32) (result i32)
           (i32.add (local.get 0)
                    (local.get 1)))
)"#;
    let mut ctx = WasmerContext::new(&store, ());

    let import_object = imports! {};

    let module = Module::new(&store, wat).unwrap();
    let instance = Instance::new(&mut ctx, &module, &import_object)?;

    let f: TypedFunction<(i32, i32), i32> = instance.exports.get_typed_function(&mut ctx, "add")?;
    f.call(&mut ctx, 4, 6)?;
    Ok(())
}

fn run_loop(mut config: crate::Config, limit: u64, iter_count: i32) -> Result<()> {
    config
        .middlewares
        .push(Arc::new(Metering::new(limit, cost_always_one)));
    let store = config.store();
    let wat = r#"(module
        (func (export "test") (param i32)
           (local i32)
           (local.set 1 (i32.const 0))
           (loop
            (local.get 1)
            (i32.const 1)
            (i32.add)
            (local.tee 1)
            (local.get 0)
            (i32.ne)
            (br_if 0)
           )
        )
)"#;
    let module = Module::new(&store, wat).unwrap();
    let mut ctx = WasmerContext::new(&store, ());

    let import_object = imports! {};

    let instance = Instance::new(&mut ctx, &module, &import_object)?;

    let f: TypedFunction<i32, ()> = instance.exports.get_typed_function(&mut ctx, "test")?;
    f.call(&mut ctx, iter_count)?;
    Ok(())
}

#[compiler_test(metering)]
fn metering_ok(config: crate::Config) -> Result<()> {
    assert!(run_add_with_limit(config, 4).is_ok());
    Ok(())
}

#[compiler_test(metering)]
fn metering_fail(config: crate::Config) -> Result<()> {
    assert!(run_add_with_limit(config, 3).is_err());
    Ok(())
}

#[compiler_test(metering)]
fn loop_once(config: crate::Config) -> Result<()> {
    assert!(run_loop(config.clone(), 12, 1).is_ok());
    assert!(run_loop(config, 11, 1).is_err());
    Ok(())
}

#[compiler_test(metering)]
fn loop_twice(config: crate::Config) -> Result<()> {
    assert!(run_loop(config.clone(), 19, 2).is_ok());
    assert!(run_loop(config, 18, 2).is_err());
    Ok(())
}

/// Ported from https://github.com/wasmerio/wasmer/blob/master/tests/middleware_common.rs
#[compiler_test(metering)]
fn complex_loop(mut config: crate::Config) -> Result<()> {
    // Assemblyscript
    // export function add_to(x: i32, y: i32): i32 {
    //    for(var i = 0; i < x; i++){
    //      if(i % 1 == 0){
    //        y += i;
    //      } else {
    //        y *= i
    //      }
    //    }
    //    return y;
    // }
    static WAT: &str = r#"
    (module
        (type $t0 (func (param i32 i32) (result i32)))
        (type $t1 (func))
        (func $add_to (export "add_to") (type $t0) (param $p0 i32) (param $p1 i32) (result i32)
        (local $l0 i32)
        block $B0
            i32.const 0
            set_local $l0
            loop $L1
            get_local $l0
            get_local $p0
            i32.lt_s
            i32.eqz
            br_if $B0
            get_local $l0
            i32.const 1
            i32.rem_s
            i32.const 0
            i32.eq
            if $I2
                get_local $p1
                get_local $l0
                i32.add
                set_local $p1
            else
                get_local $p1
                get_local $l0
                i32.mul
                set_local $p1
            end
            get_local $l0
            i32.const 1
            i32.add
            set_local $l0
            br $L1
            unreachable
            end
            unreachable
        end
        get_local $p1)
        (func $f1 (type $t1))
        (table $table (export "table") 1 anyfunc)
        (memory $memory (export "memory") 0)
        (global $g0 i32 (i32.const 8))
        (elem (i32.const 0) $f1))
    "#;
    config
        .middlewares
        .push(Arc::new(Metering::new(100, cost_always_one)));
    let store = config.store();
    let mut ctx = WasmerContext::new(&store, ());

    let module = Module::new(&store, WAT).unwrap();

    let import_object = imports! {};

    let instance = Instance::new(&mut ctx, &module, &import_object)?;

    let f: TypedFunction<(i32, i32), i32> =
        instance.exports.get_typed_function(&mut ctx, "add_to")?;

    // FIXME: Since now a metering error is signaled with an `unreachable`, it is impossible to verify
    // the error type. Fix this later.
    f.call(&mut ctx, 10_000_000, 4).unwrap_err();
    Ok(())
}
