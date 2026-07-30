use zhc_utils_macro::existential_enum;

/// Mirrors the real use case: heterogeneous payloads, acronym-heavy variant names.
#[existential_enum]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Artifact {
    Builder(String),
    IopLang(Vec<u32>),
    HpuLangTranslated(u64),
    HTMLFile(char),
}

#[existential_enum]
#[derive(Debug)]
enum Single {
    Only(u8),
}

/// Generics and where clauses must be threaded through to the generated impl block.
#[existential_enum]
#[derive(Debug)]
enum Generic<T>
where
    T: Clone,
{
    Owned(T),
    Many(Vec<T>),
}

#[test]
fn names_follow_snake_case_with_acronyms() {
    assert_eq!(Artifact::Builder("x".into()).unwrap_builder(), "x");
    assert_eq!(Artifact::IopLang(vec![1]).unwrap_iop_lang(), vec![1]);
    assert_eq!(
        Artifact::HpuLangTranslated(7).unwrap_hpu_lang_translated(),
        7
    );
    assert_eq!(Artifact::HTMLFile('c').unwrap_html_file(), 'c');
}

#[test]
fn ref_and_mut_projections() {
    let mut artifact = Artifact::IopLang(vec![1, 2]);
    assert_eq!(artifact.unwrap_iop_lang_ref(), &[1, 2]);
    artifact.unwrap_iop_lang_mut().push(3);
    assert_eq!(artifact, Artifact::IopLang(vec![1, 2, 3]));
}

#[test]
fn variant_name_reports_the_type_tag() {
    assert_eq!(Artifact::Builder("x".into()).variant_name(), "Builder");
    assert_eq!(Artifact::HTMLFile('c').variant_name(), "HTMLFile");
}

#[test]
fn tests_discriminate_the_held_variant() {
    let artifact = Artifact::IopLang(vec![1]);
    assert!(artifact.is_iop_lang());
    assert!(!artifact.is_builder());
    assert!(!artifact.is_hpu_lang_translated());
    assert!(!artifact.is_html_file());
    assert!(Artifact::HTMLFile('c').is_html_file());
}

#[test]
fn single_variant_enum_needs_no_fallback_arm() {
    assert_eq!(Single::Only(3).unwrap_only(), 3);
    assert!(Single::Only(3).is_only());
}

#[test]
fn generics_are_forwarded() {
    let mut owned = Generic::Owned(1u8);
    *owned.unwrap_owned_mut() += 1;
    assert_eq!(owned.unwrap_owned(), 2);
    assert_eq!(Generic::Many(vec!['a']).unwrap_many(), vec!['a']);
    assert!(Generic::Many(vec!['a']).is_many());
}

#[test]
#[should_panic(expected = "called `unwrap_builder` on the `IopLang` variant of `Artifact`")]
fn mismatched_projection_names_both_sides() {
    Artifact::IopLang(vec![]).unwrap_builder();
}

#[test]
#[should_panic(expected = "called `unwrap_iop_lang_mut` on the `Builder` variant of `Artifact`")]
fn mismatched_mut_projection_names_both_sides() {
    Artifact::Builder(String::new()).unwrap_iop_lang_mut();
}
