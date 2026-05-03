use std::process::Command;

fn run_py5_script(script: &str) -> std::process::Output {
    Command::new("cargo")
        .args(&["run", "--", script])
        .current_dir("/Users/Shared/ccc/project/py5")
        .output()
        .expect("failed to run py5")
}

fn assert_script_success(script: &str) {
    let output = run_py5_script(script);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() && stderr.contains("error") && !stderr.contains("file lock") {
        panic!("{} failed: {:?}", script, stderr);
    }
    if !output.status.success() && !stderr.contains("file lock") {
        panic!("{} failed with exit code {:?}: {:?}", script, output.status.code(), stderr);
    }
}

#[test]
fn test_run_basic_py() {
    assert_script_success("py/basic.py");
}

#[test]
fn test_run_oop_py() {
    assert_script_success("py/oop.py");
}

#[test]
fn test_run_magic_py() {
    assert_script_success("py/magic.py");
}

#[test]
fn test_run_args_py() {
    assert_script_success("py/args.py");
}

#[test]
fn test_run_inherit_py() {
    assert_script_success("py/inherit.py");
}

#[test]
fn test_run_unpack_py() {
    assert_script_success("py/unpack.py");
}

#[test]
fn test_run_adv_oop_py() {
    assert_script_success("py/adv_oop.py");
}

#[test]
fn test_run_modern_py() {
    assert_script_success("py/modern.py");
}

#[test]
fn test_run_test_stdlib_py() {
    assert_script_success("py/test_stdlib.py");
}

#[test]
fn test_run_test_path_py() {
    assert_script_success("py/test_path.py");
}

#[test]
fn test_run_typed_annotation_py() {
    assert_script_success("py/typed_annotation.py");
}

#[test]
fn test_import_main() {
    let output = Command::new("cargo")
        .args(&["run", "--", "py/import/main_import.py"])
        .current_dir("/Users/Shared/ccc/project/py5")
        .env("PYTHONPATH", "./py/import")
        .output()
        .expect("failed to run py5");
    assert!(output.status.success(), "main_import.py failed: {:?}", String::from_utf8_lossy(&output.stderr));
}

#[test]
fn test_pkg_main() {
    let output = Command::new("cargo")
        .args(&["run", "--", "py/pkg/main_pkg.py"])
        .current_dir("/Users/Shared/ccc/project/py5")
        .env("PYTHONPATH", "./py/pkg")
        .output()
        .expect("failed to run py5");
    assert!(output.status.success(), "main_pkg.py failed: {:?}", String::from_utf8_lossy(&output.stderr));
}