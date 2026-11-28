//! Scope: the corpus the parity tests compare (requirements PAR-01 and PAR-08): the shared device
//! library and the six twins exist beside their `.cir`, each builds with no diagnostic, each top
//! module names its nodes as the `.cir` does, and the six `.cir` files are byte-identical to the
//! content recorded here.

use std::path::PathBuf;

use piperine_lang::pom::{Design, Reflect};

/// One fixture of the corpus: its name, the top module of its twin, the node names the `.cir` uses
/// and the length and FNV-1a 64-bit hash of the `.cir` as recorded.
struct PinnedTwin {
    name: &'static str,
    top: &'static str,
    nodes: &'static [&'static str],
    cir_len: usize,
    cir_hash: u64,
}

impl PinnedTwin {
    const CORPUS: [PinnedTwin; 6] = [
        PinnedTwin { name: "divider", top: "Divider", nodes: &["vin", "mid"], cir_len: 104, cir_hash: 0x2f3f_d602_5d58_c5ff },
        PinnedTwin { name: "diode_iv", top: "DiodeIv", nodes: &["vin", "a"], cir_len: 204, cir_hash: 0xcc21_858d_eaee_1c5c },
        PinnedTwin { name: "diode_series", top: "DiodeSeries", nodes: &["vin", "a", "b"], cir_len: 143, cir_hash: 0x1a5a_30c8_e33a_cc25 },
        PinnedTwin { name: "ac_rc", top: "AcRc", nodes: &["vin", "out"], cir_len: 444, cir_hash: 0x8e02_2cbb_257a_8509 },
        PinnedTwin { name: "tran_rc", top: "TranRc", nodes: &["vin", "out"], cir_len: 634, cir_hash: 0x6ee1_832a_fc61_ae3a },
        PinnedTwin { name: "isrc_r", top: "IsrcR", nodes: &["a"], cir_len: 145, cir_hash: 0x360b_0e01_bc89_c304 },
    ];

    fn directory() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/e2e/twins")
    }

    fn cir(&self) -> PathBuf {
        Self::directory().join(format!("{}.cir", self.name))
    }

    fn twin(&self) -> PathBuf {
        Self::directory().join(format!("{}.msl", self.name))
    }

    fn fnv1a(bytes: &[u8]) -> u64 {
        bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| (hash ^ u64::from(*byte)).wrapping_mul(0x0100_0000_01b3))
    }

    fn build(&self) -> Design {
        Design::builder().root_file(self.twin()).build()
    }
}

#[test]
fn the_corpus_holds_the_library_and_the_twins() {
    let library = PinnedTwin::directory().join("devices.msl");
    let text = std::fs::read_to_string(&library).expect("tests/e2e/twins/devices.msl exists");
    for module in ["Resistor", "Capacitor", "Vsource", "Isource", "Diode"] {
        assert!(text.contains(&format!("module {module}")), "devices.msl lacks module {module}");
    }
    for fixture in &PinnedTwin::CORPUS {
        assert!(fixture.twin().is_file(), "{} has no twin {}", fixture.name, fixture.twin().display());
        assert!(fixture.cir().is_file(), "{} has no .cir {}", fixture.name, fixture.cir().display());
    }
}

#[test]
fn every_twin_builds_without_diagnostics() {
    for fixture in &PinnedTwin::CORPUS {
        let design = fixture.build();
        let found: Vec<String> = design.diagnostics().iter().map(|d| d.diagnostic.code.to_string()).collect();
        assert!(found.is_empty(), "{} builds with diagnostics {found:?}", fixture.name);
    }
}

#[test]
fn top_nodes_carry_the_cir_names() {
    for fixture in &PinnedTwin::CORPUS {
        let design = fixture.build();
        let top = design.module(fixture.top).unwrap_or_else(|| panic!("{} has no module {}", fixture.name, fixture.top));
        let mut found: Vec<&str> = top.signals().into_iter().filter_map(|s| s.name()).collect();
        let mut expected: Vec<&str> = fixture.nodes.to_vec();
        found.sort_unstable();
        expected.sort_unstable();
        assert_eq!(found, expected, "{}: signals of {} differ from the nodes of the .cir", fixture.name, fixture.top);
    }
}

#[test]
fn the_cir_fixtures_are_byte_identical() {
    for fixture in &PinnedTwin::CORPUS {
        let bytes = std::fs::read(fixture.cir()).expect("read the .cir");
        assert_eq!(bytes.len(), fixture.cir_len, "{}.cir changed length", fixture.name);
        assert_eq!(PinnedTwin::fnv1a(&bytes), fixture.cir_hash, "{}.cir changed content", fixture.name);
    }
}

#[test]
fn the_current_source_twin_exists() {
    let fixture = &PinnedTwin::CORPUS[5];
    assert_eq!(fixture.name, "isrc_r");
    let cir = std::fs::read_to_string(fixture.cir()).expect("read isrc_r.cir");
    assert!(cir.contains("print v(a)"), "isrc_r.cir prints v(a)");
    assert!(fixture.twin().is_file(), "isrc_r.msl exists");
}
