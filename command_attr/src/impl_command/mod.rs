use proc_macro2::{Ident, TokenStream};
use quote::{format_ident, quote, ToTokens};
use syn::spanned::Spanned;
use syn::{parse2, Attribute, Error, FnArg, ItemFn, Path, Result, Type};

use crate::paths;
use crate::utils::{self, AttributeArgs};

mod options;

use options::Options;

pub fn impl_command(attr: TokenStream, input: TokenStream) -> Result<TokenStream> {
    let mut fun = parse2::<ItemFn>(input)?;

    let names = if attr.is_empty() {
        vec![fun.sig.ident.to_string()]
    } else {
        parse2::<AttributeArgs>(attr)?.0
    };

    let (ctx_param, msg_param) = utils::get_first_two_parameters(&fun.sig)?;
    let ctx_param = utils::get_pat_type(&ctx_param)?.clone();
    let msg_param = utils::get_pat_type(&msg_param)?.clone();

    let options = Options::parse(&mut fun.attrs)?;

    inject_argument_parsing_code(
        utils::get_ident(&ctx_param.pat)?,
        utils::get_ident(&msg_param.pat)?,
        &mut fun,
        &options,
    )?;

    let builder_fn = builder_fn(&ctx_param.ty, &mut fun, names, &options);

    let hook_macro = paths::hook_macro();

    let result = quote! {
        #builder_fn

        #[#hook_macro]
        #[doc(hidden)]
        #fun
    };

    Ok(result)
}

/// Replace the passed in function with a "builder function" that points to the renamed original
/// function
fn builder_fn(
    ctx_ty: &Type,
    function: &mut ItemFn,
    mut names: Vec<String>,
    options: &Options,
) -> TokenStream {
    let name = names.remove(0);
    let aliases = names;

    // Derive the name of the builder from the command function.
    // Prepend the command function's name with an underscore to avoid name
    // collisions.
    let builder_name = function.sig.ident.clone();
    let function_name = format_ident!("_{}", builder_name);
    function.sig.ident = function_name.clone();

    let command_builder = paths::command_builder_type();
    let command = paths::command_type(ctx_ty);

    let vis = &function.vis;
    let external = &function.attrs;

    quote! {
        #(#external)*
        #vis fn #builder_name() -> #command {
            #command_builder::new(#name)
                #(.name(#aliases))*
                .function(#function_name)
                #options
                .build()
        }
    }
}

// Injects a code block at the top of the user-written command function that parses the command
// parameters from the FrameworkContext and Message function arguments
fn inject_argument_parsing_code(
    ctx_name: Ident,
    msg_name: Ident,
    function: &mut ItemFn,
    options: &Options,
) -> Result<()> {
    let mut arguments = Vec::new();
    while function.sig.inputs.len() > 2 {
        let argument = function.sig.inputs.pop().unwrap().into_value();

        arguments.push(Argument::new(argument)?);
    }

    // If this command has no parameters, don't bother injecting any parsing code
    if arguments.is_empty() {
        return Ok(());
    }

    arguments.reverse();

    validate_arguments_order(&arguments)?;

    let delimiter = options.delimiter.as_ref().map_or(" ", String::as_str);
    let asegsty = paths::argument_segments_type();

    let b = &function.block;

    let argument_names = arguments.iter().map(|arg| &arg.name).collect::<Vec<_>>();
    let argument_tys = arguments.iter().map(|arg| &arg.ty).collect::<Vec<_>>();
    let argument_kinds = arguments.iter().map(|arg| &arg.kind).collect::<Vec<_>>();

    function.block = parse2(quote! {{
        let (#(#argument_names),*) = {
            // Place the segments into its scope to allow mutation of `Context::args`
            // afterwards, as `ArgumentSegments` holds a reference to the source string.
            let mut __args = #asegsty::new(&#ctx_name.args, #delimiter);

            #(let #argument_names: #argument_tys = #argument_kinds(
                &#ctx_name.serenity_ctx,
                &#msg_name,
                &mut __args
            ).await?;)*

            (#(#argument_names),*)
        };

        #b
    }})?;

    Ok(())
}

/// Returns a result indicating whether the list of arguments is valid.
///
/// Valid is defined as:
/// - a list of arguments that have required arguments first,
/// optional arguments second, and variadic arguments third; one or two of these
/// types of arguments can be missing.
/// - a list of arguments that only has one variadic argument parameter, if present.
/// - a list of arguments that only has one rest argument parameter, if present.
/// - a list of arguments that only has one variadic argument parameter or one rest
/// argument parameter.
fn validate_arguments_order(args: &[Argument]) -> Result<()> {
    let mut last_arg: Option<&Argument> = None;

    for arg in args {
        if let Some(last_arg) = last_arg {
            match (last_arg.kind, arg.kind) {
                (ArgumentType::Optional, ArgumentType::Required) => {
                    return Err(Error::new(
                        last_arg.name.span(),
                        "optional argument cannot precede a required argument",
                    ));
                },
                (ArgumentType::Variadic, ArgumentType::Required) => {
                    return Err(Error::new(
                        last_arg.name.span(),
                        "variadic argument cannot precede a required argument",
                    ));
                },
                (ArgumentType::Variadic, ArgumentType::Optional) => {
                    return Err(Error::new(
                        last_arg.name.span(),
                        "variadic argument cannot precede an optional argument",
                    ));
                },
                (ArgumentType::Rest, ArgumentType::Required) => {
                    return Err(Error::new(
                        last_arg.name.span(),
                        "rest argument cannot precede a required argument",
                    ));
                },
                (ArgumentType::Rest, ArgumentType::Optional) => {
                    return Err(Error::new(
                        last_arg.name.span(),
                        "rest argument cannot precede an optional argument",
                    ));
                },
                (ArgumentType::Rest, ArgumentType::Variadic) => {
                    return Err(Error::new(
                        last_arg.name.span(),
                        "a rest argument cannot be used alongside a variadic argument",
                    ));
                },
                (ArgumentType::Variadic, ArgumentType::Rest) => {
                    return Err(Error::new(
                        last_arg.name.span(),
                        "a variadic argument cannot be used alongside a rest argument",
                    ));
                },
                (ArgumentType::Variadic, ArgumentType::Variadic) => {
                    return Err(Error::new(
                        arg.name.span(),
                        "a command cannot have two variadic argument parameters",
                    ));
                },
                (ArgumentType::Rest, ArgumentType::Rest) => {
                    return Err(Error::new(
                        arg.name.span(),
                        "a command cannot have two rest argument parameters",
                    ));
                },
                (ArgumentType::Required, ArgumentType::Required)
                | (ArgumentType::Optional, ArgumentType::Optional)
                | (ArgumentType::Required, ArgumentType::Optional)
                | (ArgumentType::Required, ArgumentType::Variadic)
                | (ArgumentType::Optional, ArgumentType::Variadic)
                | (ArgumentType::Required, ArgumentType::Rest)
                | (ArgumentType::Optional, ArgumentType::Rest) => {},
            };
        }

        last_arg = Some(arg);
    }

    Ok(())
}

struct Argument {
    name: Ident,
    ty: Box<Type>,
    kind: ArgumentType,
}

impl Argument {
    fn new(arg: FnArg) -> Result<Self> {
        let binding = utils::get_pat_type(&arg)?;

        let name = utils::get_ident(&binding.pat)?;

        let ty = binding.ty.clone();

        let path = utils::get_path(&ty)?;
        let kind = ArgumentType::new(&binding.attrs, path)?;

        Ok(Self {
            name,
            ty,
            kind,
        })
    }
}

#[derive(Clone, Copy)]
enum ArgumentType {
    Required,
    Optional,
    Variadic,
    Rest,
}

impl ArgumentType {
    fn new(attrs: &[Attribute], path: &Path) -> Result<Self> {
        if !attrs.is_empty() {
            if attrs.len() > 1 {
                return Err(Error::new(
                    path.span(),
                    "an argument cannot have more than 1 attribute",
                ));
            }

            let attr = utils::parse_attribute(&attrs[0])?;

            if !attr.path.is_ident("rest") {
                return Err(Error::new(attrs[0].span(), "invalid attribute name, expected `rest`"));
            }

            if !attr.values.is_empty() {
                return Err(Error::new(
                    attrs[0].span(),
                    "the `rest` attribute does not accept any input",
                ));
            }

            return Ok(ArgumentType::Rest);
        }

        Ok(match path.segments.last().unwrap().ident.to_string().as_str() {
            "Option" => ArgumentType::Optional,
            "Vec" => ArgumentType::Variadic,
            _ => ArgumentType::Required,
        })
    }
}

impl ToTokens for ArgumentType {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let path = match self {
            ArgumentType::Required => paths::required_argument_func(),
            ArgumentType::Optional => paths::optional_argument_func(),
            ArgumentType::Variadic => paths::variadic_arguments_func(),
            ArgumentType::Rest => paths::rest_argument_func(),
        };

        tokens.extend(quote!(#path));
    }
}
