use backend::JsCompiler;
use std::path::PathBuf;
#[test]
fn object_and_func() {
    let path = PathBuf::from("slynx/test.slx");
    let ir = slynx::compile_to_ir(path).unwrap();
    JsCompiler::compile(ir, "test-outputs/test.js".into()).unwrap();
}
