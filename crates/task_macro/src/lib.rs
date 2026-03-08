use proc_macro::TokenStream;
use quote::quote;
use syn::Ident;
use syn::LitStr;
use syn::Token;
use syn::Type;
use syn::parse::Parse;
use syn::parse::ParseStream;
use syn::parse_macro_input;
use syn::punctuated::Punctuated;

struct TaskArg {
    var: Ident,
    _colon: Token![:],
    ty: Type,
    _eq: Token![=],
    arg_name: LitStr,
}

impl Parse for TaskArg {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(TaskArg {
            var: input.parse()?,
            _colon: input.parse()?,
            ty: input.parse()?,
            _eq: input.parse()?,
            arg_name: input.parse()?,
        })
    }
}

struct AddTaskInput {
    name: LitStr,
    _comma1: Token![,],
    fun: Ident,
    _comma2: Option<Token![,]>,
    args: Punctuated<TaskArg, Token![,]>,
}

impl Parse for AddTaskInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name = input.parse()?;
        let _comma1 = input.parse()?;
        let fun = input.parse()?;

        // 允许可选的逗号
        let _comma2: Option<Token![,]> = input.parse()?;

        let args = if input.is_empty() {
            Punctuated::new()
        } else {
            Punctuated::parse_terminated(input)?
        };

        Ok(AddTaskInput {
            name,
            _comma1,
            fun,
            _comma2,
            args,
        })
    }
}

#[proc_macro]
pub fn add_task(input: TokenStream) -> TokenStream {
    let AddTaskInput {
        name, fun, args, ..
    } = parse_macro_input!(input as AddTaskInput);

    let mut arg_names = Vec::new();
    let mut extractions = Vec::new();
    let mut vars = Vec::new();

    for arg in args {
        let var = &arg.var;
        let ty = &arg.ty;
        let arg_name = &arg.arg_name;

        vars.push(var.clone());

        let is_bool = if let Type::Path(type_path) = ty {
            type_path.path.is_ident("bool")
        } else {
            false
        };

        let is_optional = var.to_string().starts_with('_');

        let extraction = if is_bool {
            arg_names.push(LitStr::new(
                &format!("?{}", arg_name.value()),
                arg_name.span(),
            ));

            quote! {
                let #var: bool = arg.get_flag(#arg_name);
            }
        } else if is_optional {
            arg_names.push(LitStr::new(
                &format!("_{}", arg_name.value()),
                arg_name.span(),
            ));

            quote! {
                let #var: Option<#ty> = arg
                    .get_one::<#ty>(#arg_name)
                    .map(|v| v.to_owned());
            }
        } else {
            arg_names.push(arg_name.clone());
            quote! {
                let #var: #ty = arg
                    .get_one::<#ty>(#arg_name)
                    .expect("missing required argument")
                    .to_owned();
            }
        };

        extractions.push(extraction);
    }

    let expanded = quote! {
        inventory::submit! {
            #[allow(unused)]
            crate::task::Task {
                name: #name,
                args: {
                    const ARGS: &[&str] = &[#(#arg_names),*];
                    ARGS
                },
                run: |arg: &clap::ArgMatches| {
                    #(#extractions)*

                    Box::pin(async move {
                        anyhow::Context::context(
                            #fun(#(#vars),*).await,
                            #name,
                        ).unwrap();
                    })
                }
            }
        }
    };

    TokenStream::from(expanded)
}
