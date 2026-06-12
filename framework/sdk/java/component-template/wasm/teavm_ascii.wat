;; Minimal TeaVM `teavm` import adapter (ASCII case only) for `wasm-tools component new --adapt`.
(module
  (func $ascii_lower (param $c i32) (result i32)
    (local $is_upper i32)
    local.get $c
    local.tee $c
    i32.const 65
    i32.ge_u
    local.get $c
    i32.const 90
    i32.le_u
    i32.and
    if (result i32)
      local.get $c
      i32.const 32
      i32.add
    else
      local.get $c
    end)

  (func $ascii_upper (param $c i32) (result i32)
    local.get $c
    local.tee $c
    i32.const 97
    i32.ge_u
    local.get $c
    i32.const 122
    i32.le_u
    i32.and
    if (result i32)
      local.get $c
      i32.const 32
      i32.sub
    else
      local.get $c
    end)

  (export "towlower" (func $ascii_lower))
  (export "towupper" (func $ascii_upper))
)
