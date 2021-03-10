use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse2, Path, Type};

fn to_path(tokens: TokenStream) -> Path {
    parse2(tokens).unwrap()
}

pub fn command_type(ctx: &Type) -> Path {
    to_path(quote! {
        serenity_framework::command::Command<
            <#ctx as serenity_framework::_DataErrorHack>::D,
            <#ctx as serenity_framework::_DataErrorHack>::E,
        >
    })
}

pub fn command_builder_type() -> Path {
    to_path(quote! {
        serenity_framework::command::CommandBuilder
    })
}

pub fn hook_macro() -> Path {
    to_path(quote! {
        serenity_framework::prelude::hook
    })
}

pub fn argument_segments_type() -> Path {
    to_path(quote! {
        serenity_framework::utils::ArgumentSegments
    })
}

pub fn required_argument_from_str_func() -> Path {
    to_path(quote! {
        serenity_framework::argument::required_argument_from_str
    })
}

pub fn required_argument_parse_func() -> Path {
    to_path(quote! {
        serenity_framework::argument::required_argument_parse
    })
}

pub fn optional_argument_from_str_func() -> Path {
    to_path(quote! {
        serenity_framework::argument::optional_argument_from_str
    })
}

pub fn optional_argument_parse_func() -> Path {
    to_path(quote! {
        serenity_framework::argument::optional_argument_parse
    })
}

pub fn variadic_arguments_from_str_func() -> Path {
    to_path(quote! {
        serenity_framework::argument::variadic_arguments_from_str
    })
}

pub fn variadic_arguments_parse_func() -> Path {
    to_path(quote! {
        serenity_framework::argument::variadic_arguments_parse
    })
}

pub fn rest_argument_from_str_func() -> Path {
    to_path(quote! {
        serenity_framework::argument::rest_argument_from_str
    })
}

pub fn rest_argument_parse_func() -> Path {
    to_path(quote! {
        serenity_framework::argument::rest_argument_parse
    })
}

pub fn check_type(ctx: &Type) -> Path {
    to_path(quote! {
        serenity_framework::check::Check<
            <#ctx as serenity_framework::_DataErrorHack>::D,
            <#ctx as serenity_framework::_DataErrorHack>::E,
        >
    })
}

pub fn check_builder_type() -> Path {
    to_path(quote! {
        serenity_framework::check::CheckBuilder
    })
}
