use std::collections::HashMap;

use convert_case::{Case, Casing};
use include_dir::{include_dir, Dir};
use serde::Serialize;
use strum::VariantNames;
use tera::{Tera, Value};

use crate::{Meta, ReflectionStrategy, TemplateKind};

// load the built-in templates statically
pub static TEMPLATE_DIR: Dir = include_dir!("$CARGO_MANIFEST_DIR/templates");

#[derive(Serialize)]
pub(crate) struct TemplateContext {
    pub(crate) crate_name: String,
    pub(crate) items: Vec<Item>,
}

#[derive(Serialize)]
pub(crate) struct Item {
    pub(crate) import_path: String,
    pub(crate) ident: String,
    pub(crate) has_static_methods: bool,
    pub(crate) variants: Vec<Variant>,
    pub(crate) functions: Vec<Function>,
    pub(crate) docstrings: Vec<String>,
    pub(crate) is_enum: bool,
    pub(crate) is_tuple_struct: bool,
    pub(crate) impls_clone: bool,
    pub(crate) impls_debug: bool,
}

/// One of enum variants or a struct
#[derive(Serialize)]
pub(crate) struct Variant {
    pub(crate) docstrings: Vec<String>,
    /// The name of the variant if it is an enum variant or None otherwise
    pub(crate) name: Option<String>,
    pub(crate) fields: Vec<Field>,
}

#[derive(Serialize)]
pub(crate) struct Field {
    pub(crate) docstrings: Vec<String>,
    pub(crate) ident: String,
    pub(crate) ty: String,
    pub(crate) reflection_strategy: ReflectionStrategy,
}

#[derive(Serialize)]
pub(crate) struct Function {
    pub(crate) ident: String,
    pub(crate) has_self: bool,
    pub(crate) args: Vec<Arg>,
    pub(crate) output: Output,
    pub(crate) docstrings: Vec<String>,
    pub(crate) from_trait_path: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct Arg {
    /// the name of the argument as in source code
    /// None if this is a receiver, in which case ty contains the ident
    pub(crate) ident: Option<String>,

    /// the type of argument
    /// i.e. `&Vec<MyTy>`
    pub(crate) ty: String,
    pub(crate) reflection_strategy: ReflectionStrategy,
}

#[derive(Serialize)]
pub(crate) struct Output {
    pub(crate) ty: String,
    pub(crate) reflection_strategy: ReflectionStrategy,
}

#[derive(Serialize)]
pub struct Collect {
    pub crates: Vec<Crate>,
    pub api_name: String,
}

#[derive(Serialize)]
pub struct Crate {
    pub name: String,
    pub meta: Meta,
}

pub fn configure_tera(
    crate_name: &str,
    user_templates_dir: &Option<cargo_metadata::camino::Utf8PathBuf>,
) -> Tera {
    // setup tera for loading templates
    let mut tera = tera::Tera::default();
    configure_tera_env(&mut tera, crate_name);

    for template_filename in TemplateKind::VARIANTS {
        // check if this template is overwritten by the user if so don't bother loading it
        if let Some(t) = &user_templates_dir {
            let template_path = t.join(template_filename);
            if template_path.exists() {
                continue;
            }
        }

        let content = crate::TEMPLATE_DIR
            .get_file(template_filename)
            .expect("Missing template kind file in the binary")
            .contents_utf8()
            .unwrap();

        tera.add_raw_template(template_filename, content)
            .expect("Could not load built-in template");
    }

    // load user templates
    if let Some(template_dir) = &user_templates_dir {
        for entry in std::fs::read_dir(template_dir).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_file() {
                let content = std::fs::read_to_string(&path).unwrap();
                tera.add_raw_template(path.file_name().unwrap().to_str().unwrap(), &content)
                    .expect("Could not load user template");
            }
        }
    }
    tera
}

pub(crate) fn configure_tera_env(tera: &mut Tera, crate_name: &str) {
    tera.set_escape_fn(|str| str.to_owned());

    // for formatting inside comments, love the filter name as well :)
    tera.register_filter(
        "prettyplease",
        |val: &Value, args: &HashMap<String, Value>| {
            let impl_context = args.contains_key("impl_context");

            let str = if impl_context {
                format!("pub trait X {{ {} }}", expect_str(val)?)
            } else {
                expect_str(val)?.to_owned()
            };

            let file = syn::parse_file(&str)
                .map_err(|e| tera::Error::msg(e.to_string()))
                .map_err(|e| {
                    log::error!("prettyplease error on input: ```\n{}\n```", str);
                    e
                })?;

            let out = prettyplease::unparse(&file);

            if impl_context {
                Ok(Value::String(
                    out.trim_start_matches("pub trait X {")
                        .trim_end_matches('}')
                        .trim()
                        .to_owned(),
                ))
            } else {
                Ok(Value::String(out))
            }
        },
    );

    // separates input via split_at string and joins with delimeter trimming whitespace around each block
    tera.register_filter("separated", |val: &Value, args: &HashMap<String, Value>| {
        let str = expect_str(val)?;
        let delimeter = expect_str(expect_arg(args, "delimeter")?)?
            .replace("\\n", "\n")
            .replace("\\t", "\t");
        let split_at = expect_str(expect_arg(args, "split_at")?)?;
        let ignore_first = args.contains_key("ignore_first");
        let mut split = str.split(split_at);

        if ignore_first {
            split.next();
        }

        Ok(Value::String(
            split
                .map(|block| block.trim())
                .collect::<Vec<_>>()
                .join(&delimeter),
        ))
    });

    tera.register_filter("indent", |val: &Value, args: &HashMap<String, Value>| {
        let str = expect_str(val)?;
        let prefix = expect_str(expect_arg(args, "prefix")?)?
            .replace("\\n", "\n")
            .replace("\\t", "\t");

        Ok(Value::String(
            str.lines()
                .map(|block| format!("{prefix}{block}"))
                .collect::<Vec<_>>()
                .join("\n"),
        ))
    });

    tera.register_filter("prefix", |val: &Value, args: &HashMap<String, Value>| {
        let str = expect_str(val)?;
        let prefix = expect_str(expect_arg(args, "val")?)?;
        Ok(Value::String(format!("{prefix}{str}")))
    });

    tera.register_filter("prefix_lua", |val: &Value, _: &HashMap<String, Value>| {
        let str = expect_str(val)?;
        Ok(Value::String(format!("Lua{str}")))
    });

    // otherwise rust complains about the crate name being borrowed
    let crate_name = crate_name.to_owned();
    tera.register_filter(
        "prefix_cratename",
        move |val: &Value, _: &HashMap<String, Value>| {
            let str = expect_str(val)?;
            Ok(Value::String(format!("{crate_name}{str}")))
        },
    );

    fn case_from_str(case: &str) -> tera::Result<Case> {
        Ok(match case {
            "camel" => Case::Camel,
            "pascal" => Case::Pascal,
            "snake" => Case::Snake,
            "kebab" => Case::Kebab,
            "screaming_snake" => Case::ScreamingSnake,
            "lower" => Case::Lower,
            "upper" => Case::Upper,
            "title" => Case::Title,
            "toggle" => Case::Toggle,
            "upper_camel" => Case::UpperCamel,
            "upper_snake" => Case::UpperSnake,
            "train" => Case::Train,
            "upper_flat" => Case::UpperFlat,
            "flat" => Case::Flat,
            "upper_kebab" => Case::UpperKebab,
            _ => return Err(tera::Error::msg("Invalid case")),
        })
    }

    tera.register_filter(
        "convert_case",
        |val: &Value, args: &HashMap<String, Value>| {
            let str = expect_str(val)?;
            let case = expect_str(expect_arg(args, "case")?)?;
            Ok(Value::String(str.to_case(case_from_str(case)?)))
        },
    )
}

fn expect_str(value: &Value) -> tera::Result<&str> {
    value
        .as_str()
        .ok_or_else(|| tera::Error::msg("Expected string"))
}

fn expect_arg<'a>(args: &'a HashMap<String, Value>, key: &str) -> tera::Result<&'a Value> {
    args.get(key)
        .ok_or_else(|| tera::Error::msg(format!("Missing argument {}", key)))
}