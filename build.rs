//! Build mruby and link it in.
//!
//! mruby is vendored rather than depended on: no crate on the registry
//! actually builds it (the ones that look like they do expect a `libmruby.a`
//! somebody else made), and the C API we bind is a dozen functions we write
//! out ourselves in `src/sys.rs`. So this script owns the whole path — fetch
//! the source at a pinned commit, run mruby's own build with our config,
//! and hand cargo the archive.
//!
//! Building mruby needs a Ruby to drive its rake, which is mruby's own
//! requirement and not one we can wish away; the parser it uses is Prism, so
//! bison is not needed. Both are build-time only — what ships is one static
//! archive inside the extension module.
//!
//! Overrides, highest first:
//!   * `MRUBY_DIR` — a source tree that is already there, used as it is.
//!   * `JOHNNY_DSL_MRUBY_REF` — which commit to fetch when vendoring.

use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

/// The commit vendored when nothing else is named.
///
/// Pinned rather than tracking a branch: the same source has to build the
/// same engine tomorrow, and a moving parser is a moving language.
const MRUBY_REF: &str = "5993adb2b5c3b3bd52d1ed3d38a4a9c31ceac8ba";
const MRUBY_REPO: &str = "https://github.com/mruby/mruby.git";

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=build_config.rb");
    println!("cargo:rerun-if-changed=src/shim.c");
    println!("cargo:rerun-if-changed=mrbgems");
    println!("cargo:rerun-if-env-changed=MRUBY_DIR");

    let source = match env::var_os("MRUBY_DIR") {
        Some(given) => PathBuf::from(given),
        None => vendored(),
    };
    let lib = build(&source);
    shim(&source);

    println!("cargo:rustc-link-search=native={}", lib.display());
    println!("cargo:rustc-link-lib=static=mruby");
    println!("cargo:rustc-link-lib=m");
}

/// Return the vendored source tree, fetching it once if it is not there.
fn vendored() -> PathBuf {
    let root = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets the manifest dir"));
    let into = root.join("vendor").join("mruby");
    if into.join("minirake").is_file() {
        return into;
    }
    let reference = env::var("JOHNNY_DSL_MRUBY_REF").unwrap_or_else(|_| MRUBY_REF.to_string());
    std::fs::create_dir_all(&into).expect("the vendor directory can be made");

    run(Command::new("git").arg("init").arg("-q").current_dir(&into), "git init");
    let _ = Command::new("git")
        .args(["remote", "add", "origin", MRUBY_REPO])
        .current_dir(&into)
        .status();
    run(
        Command::new("git")
            .args(["fetch", "--depth", "1", "origin", &reference])
            .current_dir(&into),
        "git fetch (no network? MRUBY_DIR can point at a local mruby tree)",
    );
    run(
        Command::new("git").args(["checkout", "-q", "FETCH_HEAD"]).current_dir(&into),
        "git checkout",
    );
    into
}

/// Build mruby in place and return the directory holding `libmruby.a`.
fn build(source: &Path) -> PathBuf {
    let config = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets the manifest dir"))
        .join("build_config.rb");
    let archive = source.join("build").join("host").join("lib").join("libmruby.a");
    // Always ask the build tool, even when the archive is there: it is the
    // one that knows whether a gem's Ruby changed, and it answers in
    // milliseconds when nothing did. Skipping it while the archive exists
    // is a cache keyed on the wrong thing — every edit under `mrbgems`
    // (a word, the base class) went on running against the machine built
    // before it, and only a `cargo clean` made the change appear.
    //
    // mruby's own minirake is **serial**: 316 objects compiled one at a
    // time, minutes on a 32-core machine. Real rake's `-m` (multitask)
    // runs them in parallel — 4 seconds for the same tree, measured. Use it
    // when it is there, fall back to minirake when it is not.
    let parallel = Command::new("rake")
        .arg("-m")
        .current_dir(source)
        .env("MRUBY_CONFIG", &config)
        .status();
    let built = matches!(parallel, Ok(status) if status.success());
    if !built {
        run(
            Command::new(ruby())
                .arg("./minirake")
                .current_dir(source)
                .env("MRUBY_CONFIG", &config),
            "the mruby build (it needs ruby: brew install ruby / apt install ruby)",
        );
    }
    assert!(archive.is_file(), "mruby built but left no archive at {}", archive.display());
    archive.parent().expect("the archive has a directory").to_path_buf()
}

/// Compile the macro trampolines against mruby's own headers.
///
/// They are macros there, so a Rust-side reimplementation of the boxing
/// rules would be a copy that rots when mruby's build config changes. This
/// keeps them whatever mruby says they are.
fn shim(source: &Path) {
    println!("cargo:rerun-if-changed=src/shim.c");
    let out = PathBuf::from(env::var("OUT_DIR").expect("cargo sets the out dir"));
    let object = out.join("johnny_shim.o");
    let archive = out.join("libjohnny_shim.a");
    run(
        Command::new(env::var("CC").unwrap_or_else(|_| "cc".into()))
            .arg("-c")
            .arg("-fPIC")
            .arg("-O2")
            .arg("-I")
            .arg(source.join("include"))
            .arg("-I")
            .arg(source.join("build").join("host").join("include"))
            .arg("-o")
            .arg(&object)
            .arg("src/shim.c"),
        "compiling shim.c",
    );
    run(
        Command::new("ar").arg("crs").arg(&archive).arg(&object),
        "archiving the shim",
    );
    println!("cargo:rustc-link-search=native={}", out.display());
    println!("cargo:rustc-link-lib=static=johnny_shim");
}

/// Return the ruby to build mruby with.
fn ruby() -> String {
    env::var("RUBY").unwrap_or_else(|_| "ruby".to_string())
}

/// Run one build step, failing the build with what went wrong.
fn run(command: &mut Command, what: &str) {
    let status = command
        .status()
        .unwrap_or_else(|error| panic!("{what} would not start: {error}"));
    assert!(status.success(), "{what} failed ({status})");
}
