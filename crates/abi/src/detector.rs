//! The [`Detector`] port and the keyless mock detector.
//!
//! `Detector` is the one abstraction a host drives: text in, [`Verdict`] out.
//! The Phase-3 wasmtime runtime will implement it as a `WasmDetector`; the [`mock`] module
//! implements it natively so the whole Phase-1 pipeline runs with no runtime — and so the
//! native path and the wasm artifact share **identical** rule code (P3.4 proves they agree
//! through wasmtime).

use crate::Verdict;

/// Something that runs detection over text and returns a cited [`Verdict`].
///
/// The single seam every surface drives. Detection is a pure function of the input for a given
/// detector — no clocks, no randomness, no I/O — so the same input always yields the same
/// verdict.
pub trait Detector {
    /// Run detection over `input` and return the verdict.
    fn detect(&self, input: &str) -> Verdict;
}

/// The permanent, keyless **mock** detector — a trivial fixed-keyword rule.
///
/// It exists so every command, test, and demo runs offline with no model and no wasm runtime,
/// and so the native path and the wasm artifact share the exact same rule code. It is a
/// fixture, never a real guardrail.
#[cfg(feature = "mock")]
pub mod mock {
    use super::Detector;
    use crate::{Finding, Provenance, Span, Verdict, ABI_VERSION};

    /// Stable id reported in provenance.
    pub const ID: &str = "mock";
    /// Semver reported in provenance.
    pub const VERSION: &str = "0.1.0";
    /// Score reported for every hit (the mock is a certain rule, so 1.0).
    pub const THRESHOLD: f32 = 1.0;
    /// The keywords the mock flags — case-sensitive substring matches.
    pub const KEYWORDS: &[&str] = &["badword", "injection", "secret"];

    /// The pure mock rule: flag every occurrence of a keyword as a `keyword.<word>` finding
    /// (score 1.0, byte span), sorted by span start then label for a deterministic order.
    ///
    /// This is the exact code the wasm artifact runs, so the native and wasm verdicts are
    /// byte-identical by construction.
    #[must_use]
    pub fn detect(input: &str) -> Verdict {
        let mut findings = Vec::new();
        for &kw in KEYWORDS {
            let mut from = 0;
            while let Some(rel) = input[from..].find(kw) {
                let start = from + rel;
                let end = start + kw.len();
                // Spans are u32 (wire-stable across wasm32/host); convert without silent
                // truncation. Offsets past u32::MAX can't be represented in a Span and never occur
                // for real inputs (a scanned document fits in memory) — bail if they somehow did.
                let (Ok(start_u32), Ok(end_u32)) = (u32::try_from(start), u32::try_from(end))
                else {
                    break;
                };
                findings.push(Finding::new(
                    format!("keyword.{kw}"),
                    1.0,
                    Span::new(start_u32, end_u32),
                ));
                from = end;
            }
        }
        findings.sort_by(|a, b| {
            a.span
                .start
                .cmp(&b.span.start)
                .then_with(|| a.label.cmp(&b.label))
        });
        Verdict::new(
            ABI_VERSION,
            findings,
            Provenance::new(ID, VERSION, THRESHOLD),
        )
    }

    /// The native [`Detector`] wrapper over [`detect`] — used by the CLI (P1.5) and as the test
    /// double the wasm path is checked against (P3.4).
    #[derive(Debug, Default, Clone, Copy)]
    pub struct MockDetector;

    impl MockDetector {
        /// Construct the mock detector.
        #[must_use]
        pub fn new() -> Self {
            Self
        }
    }

    impl Detector for MockDetector {
        fn detect(&self, input: &str) -> Verdict {
            detect(input)
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn flags_each_keyword_with_byte_span() {
            let v = detect("a badword and a secret");
            assert!(v.fired());
            assert_eq!(v.findings.len(), 2);
            assert_eq!(v.findings[0].label, "keyword.badword");
            assert_eq!(v.findings[0].span, Span::new(2, 9));
        }

        #[test]
        fn clean_text_has_no_findings() {
            assert!(!detect("perfectly fine text").fired());
        }

        #[test]
        fn repeated_keyword_is_found_each_time() {
            let v = detect("badword badword");
            assert_eq!(v.findings.len(), 2);
        }
    }
}
