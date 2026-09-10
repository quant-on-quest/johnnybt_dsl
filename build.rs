//! Build mruby and link it in.
//!
//! mruby is vendored rather than depended on: no crate on the registry
//! actually builds it (the ones that look like they do expect a `libmruby.a`
//! somebody else made), and the C API we bind is a dozen functions we write
//! out ourselves in `src/sys.rs`. So this script owns the whole path — fetch
//! the source at a pinned tag, run mruby's own build with our config, and
//! hand cargo the archive.
//!
//! Building mruby needs a Ruby to drive its rake, which is mruby's own
//! requirement and not one we can wish away; the parser it uses is Prism, so
//! bison is not needed. Both are build-time only — what ships is one static
//! archive inside the extension module.
//!
//! **The C compiler is whichever one cargo is already using.** mruby would
//! otherwise guess from the platform and the ambient environment, and on
//! Windows it guesses wrong in the quiet way: a MinGW `gcc` is on the path of
//! a machine whose Rust target is MSVC, so the archive builds and then does
//! not link. The `cc` crate is the one that knows what cargo picked for this
//! target, including where MSVC lives and what environment it needs, so this
//! script asks it and tells mruby.
//!
//! Overrides, highest first:
//!   * `MRUBY_DIR` — a source tree that is already there, used as it is.
//!   * `JOHNNY_DSL_MRUBY_REF` — which tag to fetch when vendoring.

use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Which mruby to build against — a **tag**, not a commit sha.
///
/// It was a sha until 2026-09-10, when upstream stopped serving it
/// (`upload-pack: not our ref`): a sha reachable from no ref can be
/// garbage-collected, and then nobody but us can build this crate at all.
/// A release tag is immutable and always reachable.
///
/// It has to be a 4.1 tag, not 4.0.0: `Regexp` comes from `mruby-regexp`,
/// which entered `full-core` after 4.0.0 was cut, and a DSL whose words
/// cannot match a pattern is not much of a DSL (the wheel built against
/// 4.0.0 answers `uninitialized constant Regexp`).
const MRUBY_REF: &str = "4.1.0-rc";
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
    let compiler = Compiler::asked();
    let lib = build(&source, &compiler);
    shim(&source);

    println!("cargo:rustc-link-search=native={}", lib.display());
    println!("cargo:rustc-link-lib=static={}", compiler.archive_stem());
    for name in libraries(&lib, &compiler) {
        println!("cargo:rustc-link-lib={name}");
    }
}

/// Return the system libraries mruby says this archive needs.
///
/// The gems in `full-core` declare their own: the task scheduler wants
/// `winmm` for the multimedia timers, sockets want `iphlpapi` and `ws2_32`,
/// and everyone on Unix wants `m`. mruby collects those declarations and
/// writes them into `libmruby.flags.mak` next to the archive, so that file
/// is the answer — reading it means a gem added upstream tomorrow arrives
/// with its libraries, instead of arriving as five unresolved symbols.
fn libraries(lib: &Path, compiler: &Compiler) -> Vec<String> {
    let written = std::fs::read_to_string(lib.join("libmruby.flags.mak")).unwrap_or_default();
    let line = written
        .lines()
        .find_map(|line| line.strip_prefix("MRUBY_LIBS"))
        .and_then(|rest| rest.split_once('=').map(|(_, value)| value));
    let Some(line) = line else {
        // No flags file: keep the one library every Unix build needs.
        return match compiler.is_msvc {
            true => Vec::new(),
            false => vec!["m".to_string()],
        };
    };
    line.split_whitespace()
        .filter_map(|token| {
            // Either `-lfoo` (gcc, clang) or `foo.lib` (MSVC).
            let name = token
                .strip_prefix("-l")
                .or_else(|| token.strip_suffix(".lib"))
                .unwrap_or_default();
            // The archive itself is already linked, statically and by us.
            let ours = name.is_empty() || name == "mruby" || name == "libmruby";
            (!ours).then(|| name.to_string())
        })
        .collect()
}

/// What cargo is compiling C with for this target, in mruby's vocabulary.
struct Compiler {
    /// mruby toolchain name: `visualcpp`, `clang` or `gcc`.
    toolchain: &'static str,
    /// The compiler driver, absolute where the `cc` crate found one.
    command: OsString,
    /// The archiver that goes with it.
    archiver: OsString,
    /// The environment the compiler needs (MSVC's include and lib paths).
    environment: Vec<(OsString, OsString)>,
    is_msvc: bool,
    /// The cross build's name, or `None` when building for this machine.
    cross: Option<String>,
}

impl Compiler {
    /// Ask the `cc` crate what this target is built with.
    fn asked() -> Self {
        let target = env::var("TARGET").expect("cargo sets the target");
        let host = env::var("HOST").expect("cargo sets the host");
        let is_msvc = env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
        let mut asking = cc::Build::new();
        asking.target(&target).host(&host).opt_level(2).cargo_metadata(false);
        let tool = asking.get_compiler();
        let toolchain = if is_msvc {
            "visualcpp"
        } else if tool.is_like_clang() {
            "clang"
        } else {
            "gcc"
        };
        let archiver = asking.get_archiver().get_program().to_os_string();
        Self {
            toolchain,
            command: shell_safe(tool.path()),
            archiver: shell_safe(Path::new(&archiver)),
            environment: tool.env().to_vec(),
            is_msvc,
            // A target that is not this machine needs mruby's own two-build
            // shape: `mrbc` runs here, the archive is for over there.
            cross: (target != host).then_some(target),
        }
    }

    /// Return the name rustc should link the archive by.
    ///
    /// mruby writes `libmruby` plus the platform's library extension, and
    /// MSVC takes the whole stem: `static=libmruby` finds `libmruby.lib`,
    /// while `static=mruby` would look for `mruby.lib` and find nothing.
    fn archive_stem(&self) -> &'static str {
        if self.is_msvc {
            "libmruby"
        } else {
            "mruby"
        }
    }

    /// Return the directory mruby writes this build's archive into.
    fn build_name(&self) -> &str {
        self.cross.as_deref().unwrap_or("host")
    }

    /// Return the archive's file name.
    fn archive_file(&self) -> String {
        let extension = if self.is_msvc { "lib" } else { "a" };
        format!("libmruby.{extension}")
    }

    /// Put what mruby's config needs into a command's environment.
    fn tell<'a>(&self, command: &'a mut Command) -> &'a mut Command {
        command
            .env("JOHNNY_DSL_TOOLCHAIN", self.toolchain)
            .env("JOHNNY_DSL_CC", &self.command)
            .env("JOHNNY_DSL_AR", &self.archiver);
        if let Some(target) = &self.cross {
            command.env("JOHNNY_DSL_CROSS", target);
        }
        for (name, value) in &self.environment {
            command.env(name, value);
        }
        command
    }
}

/// Return a form of `program` that survives being pasted into a command line.
///
/// mruby runs its compiler by building a string and handing it to a shell,
/// without quoting the command. MSVC lives under `C:\Program Files\...`, so
/// the full path arrives as two words and the shell reports that `C:\Program`
/// is not a command. The compilers this happens to are the ones the `cc`
/// crate also hands us an environment for, PATH included, so the bare name
/// resolves to exactly the same binary.
fn shell_safe(program: &Path) -> OsString {
    match program.to_string_lossy().contains(' ') {
        true => program
            .file_name()
            .unwrap_or(program.as_os_str())
            .to_os_string(),
        false => program.as_os_str().to_os_string(),
    }
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

/// Build mruby in place and return the directory holding its archive.
fn build(source: &Path, compiler: &Compiler) -> PathBuf {
    let config = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("cargo sets the manifest dir"))
        .join("build_config.rb");
    let archive = source
        .join("build")
        .join(compiler.build_name())
        .join("lib")
        .join(compiler.archive_file());
    // Always ask the build tool, even when the archive is there: it is the
    // one that knows whether a gem's Ruby changed, and it answers in
    // milliseconds when nothing did. Skipping it while the archive exists
    // is a cache keyed on the wrong thing — every edit under `mrbgems`
    // (a word, the base class) went on running against the machine built
    // before it, and only a `cargo clean` made the change appear.
    //
    // rake is a real requirement, not a convenience: mruby's `minirake` has
    // become two lines that `exec "rake"`. It is asked for with `-m`
    // (multitask) because the build is 316 objects and serial compilation
    // takes minutes where parallel takes seconds — measured on this tree.
    //
    // It goes through `ruby -S`: on Windows `rake` is a batch file, and a
    // batch file is not something `Command` can spawn by bare name.
    let parallel = compiler
        .tell(
            Command::new(ruby())
                .args(["-S", "rake", "-m"])
                .current_dir(source)
                .env("MRUBY_CONFIG", &config),
        )
        .status();
    let built = matches!(parallel, Ok(status) if status.success());
    if !built {
        run(
            compiler.tell(
                Command::new(ruby())
                    .arg("./minirake")
                    .current_dir(source)
                    .env("MRUBY_CONFIG", &config),
            ),
            "the mruby build (it needs ruby and rake: apt install ruby rake / gem install rake)",
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
    cc::Build::new()
        .file("src/shim.c")
        .include(source.join("include"))
        .include(source.join("build").join("host").join("include"))
        .opt_level(2)
        .compile("johnny_shim");
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
