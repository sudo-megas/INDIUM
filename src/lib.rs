//! INDIUM — an archive manager for Linux on Wayland.
//!
//! The library half exists so the integration tests in `tests/` can drive the reader
//! and the store without going through the window. `main.rs` is the binary.
//!
//! Copyright © sudo-megas. GPL-3.0-only.

pub mod arch;
pub mod cli;
pub mod model;
pub mod platform;
pub mod secret;
pub mod sevenz;
pub mod tasks;
pub mod theme;
pub mod ui;
pub mod util;

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;

    /// CORE §3's table and this crate are the same list of modules.
    ///
    /// §3 tabled seven where `src/` held ten: `secret`, `util` and `sevenz` had no row, two of
    /// them since P1 and the third since P4. P17 added the `cli` row and did not notice the
    /// three already missing, which is the shape of the whole defect — a table nobody compared
    /// to anything.
    ///
    /// Both directions are reported, and both lists are built rather than typed. The `pub mod`
    /// declarations are what §3 is actually describing ("one binary crate, `indium`, with
    /// modules"), and they need no special case for `platform` and `ui` being directories: a
    /// directory module and a file module declare identically. Reading `src/` as well catches
    /// the one thing declarations cannot — a file that exists and is declared nowhere. The
    /// reverse of that is free, because a declaration with no file does not compile.
    #[test]
    fn the_architecture_table_names_every_module_and_nothing_else() {
        let declared: BTreeSet<String> = include_str!("lib.rs")
            .lines()
            .filter_map(|l| l.trim().strip_prefix("pub mod "))
            .filter_map(|l| l.strip_suffix(';'))
            .map(str::to_string)
            .collect();
        assert!(
            !declared.is_empty(),
            "no `pub mod` found in lib.rs — this test would otherwise pass by absence"
        );

        let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let on_disk: BTreeSet<String> = fs::read_dir(&src)
            .expect("src/ is readable")
            .map(|e| e.expect("a readable entry in src/").path())
            .filter_map(|p| {
                let stem = p.file_stem()?.to_str()?.to_string();
                // A directory is a module only if it holds a `mod.rs`. Cargo gives `src/bin/`
                // a meaning of its own and tools write directories here too, and neither is
                // something CORE §3 should have a row for. A `.rs` file is a module unless it
                // is one of the two crate roots, which declare rather than are declared, or a
                // dotfile — an editor's lock file is `.#theme.rs`, whose stem ends in `.rs`.
                let is_module = (p.is_dir() && p.join("mod.rs").is_file())
                    || (p.extension().is_some_and(|e| e == "rs")
                        && !stem.starts_with('.')
                        && stem != "lib"
                        && stem != "main");
                is_module.then_some(stem)
            })
            .collect();
        assert_eq!(
            declared, on_disk,
            "lib.rs declares {declared:?} and src/ holds {on_disk:?}"
        );

        let core = include_str!("../CORE.md");
        let after = core
            .split_once("## 3. ARCHITECTURE")
            .expect("CORE has a section 3 heading")
            .1;
        let tabled: BTreeSet<String> = after
            .lines()
            // Bounded at the next heading before anything else. Without it, bullet-ising §3's
            // table walks the parse into §4's keyboard table and the guard below never fires:
            // it finds fifteen non-empty "modules" named `Key`, `Enter` / `Backspace` and so
            // on, and reports the wrong thing confidently.
            .take_while(|l| !l.starts_with('#'))
            .skip_while(|l| !l.starts_with('|'))
            .take_while(|l| l.starts_with('|'))
            // Not `contains("---")`: a re-centred column writes `| :-: |`, which has no run
            // of three and would be parsed as a module named `:-:`.
            .filter(|l| !l.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')))
            .filter_map(|l| {
                let cell = l.trim().trim_matches('|').split('|').next()?;
                let name = cell.trim().trim_matches('`').to_string();
                (name != "Module").then_some(name)
            })
            .collect();
        assert!(
            !tabled.is_empty(),
            "CORE §3's module table did not parse — the heading or the table has moved"
        );

        let missing: Vec<&String> = declared.difference(&tabled).collect();
        assert!(
            missing.is_empty(),
            "this crate declares {missing:?}, and CORE §3's table has no row for them"
        );
        let phantom: Vec<&String> = tabled.difference(&declared).collect();
        assert!(
            phantom.is_empty(),
            "CORE §3's table names {phantom:?}, which is not a module in this crate"
        );
    }
}
