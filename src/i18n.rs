use std::collections::HashMap;
use std::sync::Arc;

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource, FluentValue};
use unic_langid::LanguageIdentifier;

#[derive(Clone)]
pub struct I18n {
    bundle: Arc<FluentBundle<FluentResource>>,
    fallback: Arc<FluentBundle<FluentResource>>,
}

impl I18n {
    pub fn load(lang: Option<&str>) -> Self {
        let fallback = load_bundle("en-US");
        let lang_tag = lang
            .map(|s| s.to_string())
            .or_else(|| std::env::var("LANG").ok())
            .and_then(|raw| detect_lang(&raw))
            .unwrap_or_else(|| "en-US".to_string());
        let bundle = load_bundle(&lang_tag);
        Self {
            bundle: Arc::new(bundle),
            fallback: Arc::new(fallback),
        }
    }

    pub fn text(&self, key: &str) -> String {
        self.text_args(key, None)
    }

    pub fn text_args(&self, key: &str, args: Option<FluentArgs>) -> String {
        lookup_message(&self.bundle, key, args.as_ref())
            .or_else(|| lookup_message(&self.fallback, key, args.as_ref()))
            .unwrap_or_else(|| key.to_string())
    }
}

fn detect_lang(raw: &str) -> Option<String> {
    let token = raw.split('.').next().unwrap_or(raw);
    let token = token.split('@').next().unwrap_or(token);
    let token = token.replace('_', "-");
    if token.is_empty() {
        return None;
    }
    Some(token)
}

fn load_bundle(tag: &str) -> FluentBundle<FluentResource> {
    let lang: LanguageIdentifier = tag.parse().unwrap_or_else(|_| "en-US".parse().unwrap());
    let mut bundle = FluentBundle::new(vec![lang]);
    if let Some(res) = load_embedded(tag) {
        let _ = bundle.add_resource(res);
        return bundle;
    }
    bundle
}

fn load_embedded(tag: &str) -> Option<FluentResource> {
    let tag = normalize_tag(tag);
    let source = match tag.as_str() {
        "en-US" => include_str!("../locales/en-US/ui.ftl"),
        "zh-CN" => include_str!("../locales/zh-CN/ui.ftl"),
        _ => return None,
    };
    match FluentResource::try_new(source.to_string()) {
        Ok(resource) => Some(resource),
        Err((resource, _errors)) => Some(resource),
    }
}

fn normalize_tag(tag: &str) -> String {
    let token = tag.replace('_', "-");
    if token.starts_with("zh") {
        return "zh-CN".to_string();
    }
    token
}

fn lookup_message(
    bundle: &FluentBundle<FluentResource>,
    key: &str,
    args: Option<&FluentArgs>,
) -> Option<String> {
    let msg = bundle.get_message(key)?;
    let pattern = msg.value()?;
    let mut errors = Vec::new();
    let value = bundle.format_pattern(pattern, args, &mut errors);
    if !errors.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

pub fn args_from_map(values: HashMap<&'static str, String>) -> FluentArgs<'static> {
    let mut args = FluentArgs::new();
    for (k, v) in values {
        args.set(k, FluentValue::from(v));
    }
    args
}
