use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{Data, DeriveInput, Fields, Ident, Variant, parse_macro_input};

/// Inline snapshot testing macro.
///
/// Compares `actual.to_string()` against `expected` (normalized).
/// On mismatch, records the update to `target/expect_updates/` and panics.
/// Run `cargo run --bin update-expects` to apply recorded updates.
///
/// The expected string must be a raw string literal `r#"..."#`.
#[proc_macro]
pub fn assert_display_is(input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(input with syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated);
    let mut iter = args.into_iter();

    let actual = iter
        .next()
        .expect("assert_display_is! requires two arguments: actual, expected");
    let expected = iter
        .next()
        .expect("assert_display_is! requires two arguments: actual, expected");

    if iter.next().is_some() {
        panic!("assert_display_is! takes exactly two arguments");
    }

    let expanded = quote! {
        {
            let actual_val: String = (#actual).to_string();
            let expected_val: &str = #expected;
            ::zhc_utils::assert_display::check(
                &actual_val,
                expected_val,
                file!(),
                line!(),
                column!(),
                env!("CARGO_MANIFEST_DIR"),
            );
        }
    };

    expanded.into()
}

/// Derives [`std::fmt::Display`] for an enum by printing the name of the current variant.
///
/// Payloads are ignored, so the impl needs no bounds on generic parameters: a variant `Foo(T)`
/// still formats as `Foo`.
#[proc_macro_derive(DisplayVariant)]
pub fn display_variant(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let Data::Enum(enum_data) = &input.data else {
        return syn::Error::new_spanned(&input, "DisplayVariant can only be derived for enums")
            .to_compile_error()
            .into();
    };

    let enum_name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let arms = enum_data.variants.iter().map(|variant| {
        let variant_name = &variant.ident;
        let variant_str = variant_name.to_string();
        // Payloads are irrelevant to the output, but the pattern still has to match their shape.
        let ignore = match &variant.fields {
            Fields::Unit => quote! {},
            Fields::Unnamed(_) => quote! { (..) },
            Fields::Named(_) => quote! { {..} },
        };
        quote! { Self::#variant_name #ignore => f.write_str(#variant_str) }
    });

    // A variantless enum is uninhabited: `match *self {}` is the only exhaustive body, and the
    // formatter goes unused.
    let body = if enum_data.variants.is_empty() {
        quote! { let _ = f; match *self {} }
    } else {
        quote! { match self { #(#arms,)* } }
    };

    quote! {
        impl #impl_generics ::std::fmt::Display for #enum_name #ty_generics #where_clause {
            fn fmt(&self, f: &mut ::std::fmt::Formatter<'_>) -> ::std::fmt::Result {
                #body
            }
        }
    }
    .into()
}

/// Generates projection accessors for a closed-world existential enum.
///
/// Such an enum enumerates every payload type a subsystem can carry, the discriminant playing the
/// role of a type tag (a closed [`std::any::Any`]). This macro derives, for every variant `Foo(T)`:
///
/// - `unwrap_foo(self) -> T`
/// - `unwrap_foo_ref(&self) -> &T`
/// - `unwrap_foo_mut(&mut self) -> &mut T`
///
/// plus `variant_name(&self) -> &'static str`, used to report the type tag actually found when a
/// projection fails. Accessors are `#[track_caller]`, so the panic points at the caller.
///
/// Variant names are converted to `snake_case`, acronym runs included: `HpuLangTranslated` yields
/// `unwrap_hpu_lang_translated`, `IopLang` yields `unwrap_iop_lang`.
///
/// Every variant must be a newtype variant, since a projection needs exactly one payload type to
/// return. Unit, struct and multi-field tuple variants are rejected at compile time.
#[proc_macro_attribute]
pub fn existential_enum(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let Data::Enum(enum_data) = &input.data else {
        return syn::Error::new_spanned(&input, "existential_enum can only be applied to enums")
            .to_compile_error()
            .into();
    };

    let enum_name = &input.ident;
    let vis = &input.vis;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // A single-variant enum is already matched exhaustively by its own arm: emitting the panicking
    // fallback would trip `unreachable_patterns`.
    let needs_fallback = enum_data.variants.len() > 1;

    let mut name_arms = Vec::with_capacity(enum_data.variants.len());
    let mut accessors = Vec::with_capacity(enum_data.variants.len() * 3);

    for variant in &enum_data.variants {
        let variant_name = &variant.ident;
        let variant_str = variant_name.to_string();

        // Reject anything that is not a newtype variant: there is no single payload to project.
        let field = match &variant.fields {
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => &fields.unnamed[0],
            _ => {
                return syn::Error::new_spanned(
                    variant,
                    format!(
                        "existential_enum requires every variant to hold exactly one payload; \
                         `{variant_str}` does not"
                    ),
                )
                .to_compile_error()
                .into();
            }
        };
        let ty = &field.ty;

        name_arms.push(quote! { Self::#variant_name(..) => #variant_str });

        let snake = to_snake_case(&variant_str);
        let span = variant_name.span();
        let unwrap = format_ident!("unwrap_{}", snake, span = span);
        let unwrap_ref = format_ident!("unwrap_{}_ref", snake, span = span);
        let unwrap_mut = format_ident!("unwrap_{}_mut", snake, span = span);

        // Panic messages are formatted at expansion time, `{{}}` leaving a hole for the tag found.
        let msg =
            |method: &Ident| format!("called `{method}` on the `{{}}` variant of `{enum_name}`");
        let (msg, msg_ref, msg_mut) = (msg(&unwrap), msg(&unwrap_ref), msg(&unwrap_mut));

        let fallback = |msg: String| {
            needs_fallback.then(|| quote! { other => panic!(#msg, other.variant_name()) })
        };
        let (fb, fb_ref, fb_mut) = (fallback(msg), fallback(msg_ref), fallback(msg_mut));

        let doc = format!("Moves the [`{enum_name}::{variant_str}`] payload out.");
        let doc_ref = format!("Borrows the [`{enum_name}::{variant_str}`] payload.");
        let doc_mut = format!("Mutably borrows the [`{enum_name}::{variant_str}`] payload.");
        let panics = format!("Panics if the variant is not [`{enum_name}::{variant_str}`].");

        accessors.push(quote! {
            #[doc = #doc]
            ///
            /// # Panics
            ///
            #[doc = #panics]
            #[track_caller]
            #vis fn #unwrap(self) -> #ty {
                match self {
                    Self::#variant_name(payload) => payload,
                    #fb
                }
            }

            #[doc = #doc_ref]
            ///
            /// # Panics
            ///
            #[doc = #panics]
            #[track_caller]
            #vis fn #unwrap_ref(&self) -> &#ty {
                match self {
                    Self::#variant_name(payload) => payload,
                    #fb_ref
                }
            }

            #[doc = #doc_mut]
            ///
            /// # Panics
            ///
            #[doc = #panics]
            #[track_caller]
            #vis fn #unwrap_mut(&mut self) -> &mut #ty {
                match self {
                    Self::#variant_name(payload) => payload,
                    #fb_mut
                }
            }
        });
    }

    let expanded = quote! {
        #input

        impl #impl_generics #enum_name #ty_generics #where_clause {
            /// Returns the name of the current variant, that is, the payload's type tag.
            #vis fn variant_name(&self) -> &'static str {
                match self {
                    #(#name_arms,)*
                }
            }

            #(#accessors)*
        }
    };

    expanded.into()
}

/// Converts a `PascalCase` identifier to `snake_case`, keeping acronym runs together.
///
/// An underscore is inserted before an uppercase letter that either follows a lowercase character
/// or a digit (`HpuLang` → `hpu_lang`), or that closes an acronym run by being followed by a
/// lowercase one (`HTMLFile` → `html_file`).
fn to_snake_case(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len() + 4);

    for (idx, ch) in chars.iter().enumerate() {
        if !ch.is_uppercase() {
            out.push(*ch);
            continue;
        }
        let follows_word =
            idx > 0 && (chars[idx - 1].is_lowercase() || chars[idx - 1].is_numeric());
        let ends_acronym = idx > 0
            && chars[idx - 1].is_uppercase()
            && chars.get(idx + 1).is_some_and(|next| next.is_lowercase());
        if follows_word || ends_acronym {
            out.push('_');
        }
        out.extend(ch.to_lowercase());
    }

    out
}

#[proc_macro_attribute]
pub fn fsm(_args: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // Check that it's applied to an enum
    let Data::Enum(mut enum_data) = input.data else {
        return syn::Error::new_spanned(input, "fsm can only be applied to enums")
            .to_compile_error()
            .into();
    };

    // Add __INVALID variant
    let invalid_variant = Variant {
        attrs: vec![],
        ident: Ident::new("__INVALID", proc_macro2::Span::call_site()),
        fields: Fields::Unit,
        discriminant: None,
    };
    enum_data.variants.push(invalid_variant);

    let enum_name = &input.ident;
    let vis = &input.vis;
    let attrs = &input.attrs;
    let generics = &input.generics;
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let variants = enum_data.variants.iter();

    let expanded = quote! {
        #(#attrs)*
        #vis enum #enum_name #generics {
            #(#variants,)*
        }

        impl #impl_generics #enum_name #ty_generics #where_clause {
            /// Transitions the FSM state using the provided function.
            ///
            /// The function receives the current state and must return the new state.
            /// This method safely handles the transition by temporarily setting the
            /// state to __INVALID during the transformation.
            pub fn transition<F>(&mut self, mut transitioner: F)
            where
                F: FnOnce(Self) -> Self
            {
                let old_state = std::mem::replace(self, Self::__INVALID);
                *self = transitioner(old_state);
            }

            /// Like [`transition`], but the closure returns a `(NewState, T)` pair,
            /// allowing extraction of data during the state change.
            pub fn transition_with<F, T>(&mut self, transitioner: F) -> T
            where
                F: FnOnce(Self) -> (Self, T)
            {
                let old_state = std::mem::replace(self, Self::__INVALID);
                let (new_state, val) = transitioner(old_state);
                *self = new_state;
                val
            }
        }
    };

    expanded.into()
}
