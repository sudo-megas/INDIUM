//! INDIUM — an archive manager for Linux on Wayland.
//!
//! The library half exists so the integration tests in `tests/` can drive the reader
//! and the store without going through the window. `main.rs` is the binary.
//!
//! Copyright © sudo-megas. GPL-3.0-only.

pub mod arch;
pub mod cli;
pub mod estimate;
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

    /// The line `release.yml` derives the one acceptable tag with, quoted rather than described.
    ///
    /// A test that models a rule the workflow no longer follows is the stale-prose class wearing
    /// a green tick, so the model is pinned to the text it models: change the workflow's rule and
    /// the three tests below fail until whoever changed it says what the new shapes are.
    const DERIVATION: &str =
        r#"if [ "$REL" = 1 ]; then EXPECT="v${VER%.*}"; else EXPECT="v${VER}-${REL}"; fi"#;

    /// Where a tag sits in the sequence — major, minor, patch, revision — so two can be compared.
    ///
    /// Derived rather than written, because the only ordering this needs is the lexicographic one
    /// the field order already gives.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
    struct TagOrder(u32, u32, u32, u32);

    /// A tag `release.yml` would accept, parsed into the four numbers that order it.
    ///
    /// Two shapes and no third, because the workflow writes one of exactly two strings:
    /// `v${VER%.*}` at `pkgrel` 1, and `v${VER}-${REL}` above it. So a tag with no revision has
    /// exactly two numerals, a tag with a revision has exactly three — and **a revision of 1 is
    /// unreachable**, because `pkgrel` of 1 takes the other branch. `v2.5.0-1` is therefore a
    /// tag no push could ever be accepted under, which is the trap this is built to catch.
    ///
    /// The patch numeral of a two-numeral tag is unknowable — `%.*` cut it off before the tag
    /// existed. It is modelled as 0 for ordering only, and nothing here claims to know it.
    fn parse_release_tag(tag: &str) -> Result<TagOrder, String> {
        let body = tag
            .strip_prefix('v')
            .ok_or_else(|| format!("{tag:?} does not begin with `v`"))?;
        let numerals = |s: &str| -> Result<Vec<u32>, String> {
            s.split('.')
                .map(|part| {
                    part.parse::<u32>()
                        .map_err(|_| format!("{tag:?} has a non-numeric component {part:?}"))
                })
                .collect()
        };
        match body.split_once('-') {
            Some((ver, rel)) => {
                let n = numerals(ver)?;
                if n.len() != 3 {
                    return Err(format!(
                        "{tag:?} carries a revision, so the workflow wrote `v${{VER}}-${{REL}}` \
                         with Cargo's three-numeral version; this has {}",
                        n.len()
                    ));
                }
                let r: u32 = rel
                    .parse()
                    .map_err(|_| format!("{tag:?} has a non-numeric revision {rel:?}"))?;
                if r < 2 {
                    return Err(format!(
                        "{tag:?} names revision {r}, which the workflow never writes: at \
                         `pkgrel` 1 it takes the other branch and writes the two-numeral form"
                    ));
                }
                Ok(TagOrder(n[0], n[1], n[2], r))
            }
            None => {
                let n = numerals(body)?;
                if n.len() != 2 {
                    return Err(format!(
                        "{tag:?} carries no revision, so the workflow wrote `v${{VER%.*}}`, \
                         which has exactly two numerals; this has {}",
                        n.len()
                    ));
                }
                Ok(TagOrder(n[0], n[1], 0, 1))
            }
        }
    }

    /// CORE §7's road table, as (the P column, the Tag cell), in the order the table writes them.
    ///
    /// Bounded at the `###` heading, not the `##` one: the road table lives *inside* §7, so
    /// splitting on `## 7. VERSIONS` and stopping at the next `#` stops at
    /// `### The road to v1.0` and never reaches a single row.
    fn road_table() -> Vec<(String, String)> {
        let core = include_str!("../CORE.md");
        let after = core
            .split_once("### The road to v1.0")
            .expect("CORE §7 has a road-table heading")
            .1;
        after
            .lines()
            .take_while(|l| !l.starts_with('#'))
            .skip_while(|l| !l.starts_with('|'))
            .take_while(|l| l.starts_with('|'))
            // The separator row by its character set, not by `contains("---")`: a re-centred
            // column writes `| :-: |`, which has no run of three.
            .filter(|l| !l.chars().all(|c| matches!(c, '|' | '-' | ':' | ' ')))
            .filter_map(|l| {
                let cells: Vec<&str> = l.trim().trim_matches('|').split('|').collect();
                // Three columns — P, Ships, Tag — and the Tag is taken as the last, which
                // survives a Ships cell that grows a pipe of its own. The length guard is what
                // stops a two-column row from having its Ships cell read as a tag.
                if cells.len() < 3 {
                    return None;
                }
                let p = cells[0].trim().to_string();
                let tag = cells[cells.len() - 1].trim().to_string();
                (p != "P").then_some((p, tag))
            })
            .collect()
    }

    /// A round whose tag was held writes an em dash and the reason. That is a record of a
    /// decision, not a tag, and there is nothing in it to check.
    fn held(cell: &str) -> bool {
        cell.starts_with('—')
    }

    /// Strip the table's emphasis off a tag cell. The released tags are bold, the pre-`v1.0`
    /// ones are not, and both are code-quoted.
    fn bare(cell: &str) -> &str {
        cell.trim_matches(|c| c == '*' || c == '`')
    }

    /// `parse_release_tag` accepts the two shapes the workflow writes and refuses the rest.
    ///
    /// This exists because the gate above cannot be sabotage-tested the way every other gate in
    /// this project is: proving it fires on a bad tag would mean writing a bad tag into `CORE.md`,
    /// and CORE is edited by one hand and not this one. So the refusals are pinned here instead,
    /// against literals — which is stronger anyway, because it names every shape that is wrong
    /// rather than demonstrating one.
    #[test]
    fn the_tag_parser_refuses_every_shape_the_workflow_cannot_write() {
        // The two forms, at both ends of the table's range.
        assert_eq!(parse_release_tag("v0.2"), Ok(TagOrder(0, 2, 0, 1)));
        assert_eq!(parse_release_tag("v1.0"), Ok(TagOrder(1, 0, 0, 1)));
        assert_eq!(parse_release_tag("v2.1"), Ok(TagOrder(2, 1, 0, 1)));
        assert_eq!(parse_release_tag("v1.0.0-2"), Ok(TagOrder(1, 0, 0, 2)));
        assert_eq!(parse_release_tag("v1.2.0-4"), Ok(TagOrder(1, 2, 0, 4)));
        // Nothing here is bounded to one digit.
        assert_eq!(
            parse_release_tag("v10.11.12-13"),
            Ok(TagOrder(10, 11, 12, 13))
        );

        // The trap the gate exists for: `pkgrel` of 1 takes the two-numeral branch, so no push
        // can ever be accepted under a revision of 1. This is the shape a future round is most
        // likely to write, because it is what the changelog and the `.deb` filename both use.
        assert!(parse_release_tag("v2.5.0-1").is_err());
        // Cargo's own three-numeral version, tagged as-is — the mistake `v${VER%.*}` prevents.
        assert!(parse_release_tag("v2.5.0").is_err());
        // A version of `2.5` at `pkgrel` 1 derives this, and it is not a tag §7 describes.
        assert!(parse_release_tag("v2").is_err());
        // A revision on the two-numeral form: the branches cannot be crossed.
        assert!(parse_release_tag("v2.5-2").is_err());
        // Four numerals, and the empty tail `v2.5.0.` produces.
        assert!(parse_release_tag("v1.2.3.4-2").is_err());
        assert!(parse_release_tag("v2.5.").is_err());
        // Not a tag at all.
        assert!(parse_release_tag("2.3").is_err());
        assert!(parse_release_tag("v").is_err());
        assert!(parse_release_tag("v-2").is_err());
        assert!(parse_release_tag("v2.x").is_err());
        assert!(parse_release_tag("v2.5.0-x").is_err());
        // A revision of 0 is not reachable either, for the same reason 1 is not.
        assert!(parse_release_tag("v2.5.0-0").is_err());
    }

    /// Every tag CORE §7's road table names is one `release.yml` would accept.
    ///
    /// The commitment this closes was carried unkept from PXX Phase 4, and its own wording had
    /// gone stale before anyone acted on it — it asked for *"the tenth"* doc-as-test when nine
    /// existed and eleven do now. So the ordinal is deliberately absent here: a count of tests
    /// is a number nothing can check, in the class this project has never beaten.
    ///
    /// What the gate is for: the road table is where a tag is *written down*, and `release.yml`
    /// is where a tag is *accepted*. Nothing compared them. A row naming a shape the workflow
    /// cannot produce would be discovered at the push, which is the most expensive moment
    /// available — the three build jobs run first and the gate fails last.
    #[test]
    fn every_tag_core_seven_names_is_one_the_release_workflow_would_accept() {
        let workflow = include_str!("../.github/workflows/release.yml");
        let gates: BTreeSet<&str> = workflow
            .lines()
            .map(str::trim)
            .filter(|l| l.contains("EXPECT="))
            .collect();
        assert!(
            !gates.is_empty(),
            "release.yml no longer derives an EXPECT, so this test's model of the rule has \
             nothing left to check itself against"
        );
        // Two jobs gate on the tag and each carries its own copy of the line. Two hand-copied
        // gates in one file is the sibling class, so they are compared to each other before
        // either is compared to the model.
        assert_eq!(
            gates.len(),
            1,
            "release.yml's tag gate is written more than one way, so its jobs can disagree \
             about which tag is acceptable: {gates:?}"
        );
        assert_eq!(
            *gates.iter().next().expect("exactly one distinct gate"),
            DERIVATION,
            "release.yml derives the acceptable tag differently now, so the two shapes this \
             test enforces are no longer the two shapes the workflow writes"
        );

        let rows = road_table();
        assert!(
            !rows.is_empty(),
            "CORE §7's road table did not parse — the heading or the table has moved"
        );
        let mut named = 0;
        for (p, cell) in &rows {
            if held(cell) {
                continue;
            }
            match parse_release_tag(bare(cell)) {
                Ok(_) => named += 1,
                Err(why) => {
                    panic!("CORE §7's row for {p} names a tag release.yml would refuse: {why}")
                }
            }
        }
        assert!(
            named > 0,
            "the road table parsed {} rows and named no tag at all — every cell was read as \
             held, so this test would otherwise pass by absence",
            rows.len()
        );
    }

    /// The road table never names a tag earlier than the row above it.
    ///
    /// Repeats are ordinary and are allowed: several rounds shipped under one tag, so `v1.0`
    /// appears twice and `v1.0.0-4` three times. Going *backwards* is not ordinary — it means a
    /// row was inserted in the wrong place, which is the one mistake a table that only ever
    /// grows at the bottom is exposed to.
    #[test]
    fn the_road_table_never_names_a_tag_earlier_than_the_row_above_it() {
        let mut previous: Option<(String, String, TagOrder)> = None;
        for (p, cell) in road_table() {
            if held(&cell) {
                continue;
            }
            let tag = bare(&cell).to_string();
            // Whether a tag is well-formed is the other test's verdict to render. This one
            // reports on order and stays silent about shape, so a single malformed cell fails
            // one test with one reason rather than two with two.
            let Ok(order) = parse_release_tag(&tag) else {
                continue;
            };
            if let Some((before, earlier_tag, earlier)) = &previous {
                assert!(
                    order >= *earlier,
                    "CORE §7's road table goes backwards: {before} names {earlier_tag:?}, \
                     ordering {earlier:?}, and {p} below it names {tag:?}, ordering {order:?}"
                );
            }
            previous = Some((p, tag, order));
        }
        assert!(
            previous.is_some(),
            "no tag in the road table parsed — this test would otherwise pass by absence"
        );
    }

    /// The tag this tree would be released under is one CORE §7 describes.
    ///
    /// `the_pkgbuild_and_cargo_toml_agree_about_the_version` checks that the two files hold the
    /// same version. This checks the other half: that the string the workflow *builds* out of
    /// them is a shape §7 recognises. A version of `2.5` with `pkgrel` 1 derives the tag `v2`,
    /// which agrees with itself perfectly and is not a tag this project has ever written.
    #[test]
    fn the_tag_this_tree_would_be_released_under_is_well_formed() {
        let cargo = include_str!("../Cargo.toml");
        let pkgbuild = include_str!("../build/package/PKGBUILD");
        // The first `version =` at column 0 is `[package]`'s; a dependency's is indented or
        // inline in a table.
        let ver = cargo
            .lines()
            .find_map(|l| l.strip_prefix("version = "))
            .map(|v| v.trim().trim_matches('"'))
            .expect("Cargo.toml has no top-level `version =` line");
        let rel = pkgbuild
            .lines()
            .find_map(|l| l.strip_prefix("pkgrel="))
            .map(str::trim)
            .expect("the PKGBUILD has no `pkgrel=` line");

        // The same two branches as `DERIVATION`, which is pinned above so this cannot drift
        // away from the shell that actually runs.
        let expect = if rel == "1" {
            let head = ver
                .rsplit_once('.')
                .map(|(head, _)| head)
                .unwrap_or_else(|| {
                    panic!("Cargo.toml's version {ver:?} has no dot for `%.*` to cut")
                });
            format!("v{head}")
        } else {
            format!("v{ver}-{rel}")
        };
        if let Err(why) = parse_release_tag(&expect) {
            panic!(
                "this tree would be tagged {expect:?} (Cargo.toml {ver}, pkgrel {rel}), which \
                 is not a tag CORE §7 describes: {why}"
            );
        }
    }
}
