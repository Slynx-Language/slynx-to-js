use slynx_to_js::JsCompiler;
use std::path::PathBuf;

#[test]
fn component_constructors_preserve_fields_and_children() {
    let ir =
        slynx::compile_to_ir(PathBuf::from("tests/fixtures/component_children.slx"), None).unwrap();
    let js = JsCompiler::compile(ir).unwrap();

    assert!(js.contains("Main(9,Text(S_empty()))"), "{js}");
    assert!(js.contains("function Main(p0,p1)"), "{js}");
    assert!(js.contains("return {f0: p0,c0: p1};"), "{js}");
    assert!(js.contains("function Text(p0)"), "{js}");
    assert!(js.contains("return {f0: p0};"), "{js}");
    assert!(js.contains("Text_update(p0.c0,p1)"), "{js}");
}
