use zhc_utils_macro::DisplayVariant;

/// Mirrors the real use case: a unit-only type tag enum.
#[derive(DisplayVariant, Debug, Clone, PartialEq, Eq, Hash)]
pub enum TypeSystem {
    Builder,
    IopLang,
    MultiHpuLangTranslated,
}

/// Payloads are ignored, and no `Display` bound is needed on `T`.
#[derive(DisplayVariant)]
#[allow(dead_code)] // The derive never reads the payloads, which is precisely the point.
enum Mixed<T> {
    Unit,
    Tuple(T, u8),
    Struct { field: T },
}

/// A variantless enum is uninhabited, so `fmt` is unreachable but must still compile.
#[derive(DisplayVariant)]
enum Never {}

#[test]
fn prints_the_variant_name() {
    assert_eq!(TypeSystem::Builder.to_string(), "Builder");
    assert_eq!(TypeSystem::IopLang.to_string(), "IopLang");
    assert_eq!(
        TypeSystem::MultiHpuLangTranslated.to_string(),
        "MultiHpuLangTranslated"
    );
}

#[test]
fn payloads_are_ignored_without_display_bounds() {
    // `Vec<u8>` is not `Display`, which the derive must not require.
    assert_eq!(Mixed::<Vec<u8>>::Unit.to_string(), "Unit");
    assert_eq!(Mixed::Tuple(vec![1u8], 2).to_string(), "Tuple");
    assert_eq!(Mixed::Struct { field: vec![1u8] }.to_string(), "Struct");
}

#[test]
fn honours_formatter_flags_via_display() {
    // `write_str` bypasses padding, matching the behaviour of a hand-written `write!(f, "..")`.
    assert_eq!(format!("{:>10}", TypeSystem::Builder), "Builder");
}

#[test]
fn uninhabited_enum_compiles() {
    fn assert_display<T: std::fmt::Display>() {}
    assert_display::<Never>();
}
