//! Every template the app offers (web/templates/*.easycad) opens and prints: it runs without a
//! problem, prints something, and every part's G-code and every export is made. A few seconds;
//! the quick check for a recipe change (the browser smoke test is for changes to the app).

use crate::eval::{Quality, evaluate};
use crate::files::{Which, make};
use crate::gcode::Profile;

/// The templates, by file name, with their text.
fn templates() -> Vec<(String, String)> {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../web/templates");
    let mut out: Vec<(String, String)> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.extension().is_some_and(|x| x == "easycad"))
        .map(|p| {
            let name = p.file_name().unwrap().to_string_lossy().into_owned();
            (name, std::fs::read_to_string(&p).unwrap())
        })
        .collect();
    out.sort();
    out
}

#[test]
fn every_template_opens_and_prints() {
    let all = templates();
    assert!(all.len() >= 9, "only {} templates found", all.len());
    for (name, src) in &all {
        let e = evaluate(src, Quality::Final);
        let errors: Vec<String> = e
            .errors()
            .map(|d| format!("line {}: {}", d.line, d.message))
            .collect();
        assert!(errors.is_empty(), "{name}: {errors:?}");
        assert!(!e.prints.is_empty(), "{name} prints nothing");
        // A big bed with the origin in its middle, as the app places a print.
        let profile = Profile {
            center: [500.0, 500.0],
            ..Profile::default()
        };
        for i in 0..e.prints.len() {
            let file = make(&e, Which::Print(i), &profile);
            assert!(
                file.as_ref().is_ok_and(|f| !f.bytes.is_empty()),
                "{name}, part {i}: {:?}",
                file.err()
            );
        }
        for i in 0..e.exports.len() {
            let file = make(&e, Which::Export(i), &profile);
            assert!(
                file.as_ref().is_ok_and(|f| !f.bytes.is_empty()),
                "{name}, export {i}: {:?}",
                file.err()
            );
        }
    }
}
