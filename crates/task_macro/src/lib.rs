use proc_macro::TokenStream;
use quote::quote;
use syn::FnArg;
use syn::ItemFn;
use syn::LitStr;
use syn::Pat;
use syn::Token;
use syn::Type;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::parse_macro_input;
use syn::punctuated::Punctuated;

// ── attribute arguments ──────────────────────────────────────────────

/// A single rename pair: `ident = "cli_name"`
struct RenamePair {
    ident: syn::Ident,
    _eq: Token![=],
    cli_name: LitStr,
}

impl Parse for RenamePair {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(RenamePair {
            ident: input.parse()?,
            _eq: input.parse()?,
            cli_name: input.parse()?,
        })
    }
}

/// `#[add_task("task_name", rename(a = "x", b = "y"))]`
struct MacroArgs {
    name: LitStr,
    renames: Vec<RenamePair>,
}

impl Parse for MacroArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: LitStr = input.parse()?;
        let mut renames = Vec::new();

        if input.peek(Token![,]) {
            let _: Token![,] = input.parse()?;

            if !input.is_empty() {
                // expect `rename(...)`
                let kw: syn::Ident = input.parse()?;
                if kw != "rename" {
                    return Err(syn::Error::new(kw.span(), "expected `rename`"));
                }
                let content;
                syn::parenthesized!(content in input);
                let pairs: Punctuated<RenamePair, Token![,]> =
                    Punctuated::parse_terminated(&content)?;
                renames = pairs.into_iter().collect();
            }
        }

        Ok(MacroArgs { name, renames })
    }
}

// ── helpers ──────────────────────────────────────────────────────────

fn is_bool(ty: &Type) -> bool {
    if let Type::Path(tp) = ty
        && tp.path.segments.len() == 1
    {
        return tp.path.segments[0].ident == "bool";
    }
    false
}

fn get_option_inner_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(tp) = ty
        && tp.path.segments.len() == 1
        && tp.path.segments[0].ident == "Option"
        && let syn::PathArguments::AngleBracketed(args) = &tp.path.segments[0].arguments
        && let Some(syn::GenericArgument::Type(inner)) = args.args.first()
    {
        return Some(inner);
    }
    None
}

#[allow(unused)]
fn get_vec_inner_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(tp) = ty
        && let Some(seg) = tp.path.segments.last()
        && seg.ident == "Vec"
        && let syn::PathArguments::AngleBracketed(args) = &seg.arguments
        && let Some(syn::GenericArgument::Type(inner)) = args.args.first()
    {
        return Some(inner);
    }
    None
}

// ── attribute macro ──────────────────────────────────────────────────

#[proc_macro_attribute]
pub fn add_task(attr: TokenStream, item: TokenStream) -> TokenStream {
    let MacroArgs { name, renames } = parse_macro_input!(attr as MacroArgs);
    let func = parse_macro_input!(item as ItemFn);
    let func_ident = &func.sig.ident;

    let mut arg_metas = Vec::new(); // TokenStream pieces for TaskArg array
    let mut extractions = Vec::new(); // let-bindings in the closure
    let mut call_vars = Vec::new(); // variables passed to the function call

    for fn_arg in &func.sig.inputs {
        let FnArg::Typed(pat_type) = fn_arg else {
            continue;
        };
        let Pat::Ident(pat_ident) = pat_type.pat.as_ref() else {
            continue;
        };

        let var = &pat_ident.ident;
        let ty = pat_type.ty.as_ref();

        // Determine CLI argument name: check if there is a rename for this param
        let cli_name = renames
            .iter()
            .find(|r| r.ident == *var)
            .map(|r| r.cli_name.clone())
            .unwrap_or_else(|| LitStr::new(&var.to_string(), var.span()));

        call_vars.push(var.clone());

        if is_bool(ty) {
            // bool -> Flag
            arg_metas.push(quote! {
                crate::task::TaskArg {
                    name: #cli_name,
                    kind: crate::task::ArgKind::Flag,
                }
            });
            extractions.push(quote! {
                let #var: bool = arg.get_flag(#cli_name);
            });
        } else if let Some(inner) = get_option_inner_type(ty) {
            // Option<T> -> Optional
            arg_metas.push(quote! {
                crate::task::TaskArg {
                    name: #cli_name,
                    kind: crate::task::ArgKind::Optional,
                }
            });
            extractions.push(quote! {
                let #var: Option<#inner> = arg
                    .get_one::<#inner>(#cli_name)
                    .map(|v| v.to_owned());
            });
        } else {
            // T -> Required
            arg_metas.push(quote! {
                crate::task::TaskArg {
                    name: #cli_name,
                    kind: crate::task::ArgKind::Required,
                }
            });
            extractions.push(quote! {
                let #var: #ty = arg
                    .get_one::<#ty>(#cli_name)
                    .expect("missing required argument")
                    .to_owned();
            });
        }
    }

    let expanded = quote! {
        #func

        inventory::submit! {
            #[allow(unused)]
            crate::task::Task {
                name: #name,
                args: {
                    const ARGS: &[crate::task::TaskArg] = &[#(#arg_metas),*];
                    ARGS
                },
                run: |arg: &clap::ArgMatches| {
                    #(#extractions)*

                    Box::pin(async move {
                        anyhow::Context::context(
                            #func_ident(#(#call_vars),*).await,
                            #name,
                        ).unwrap();
                    })
                }
            }
        }
    };

    TokenStream::from(expanded)
}
