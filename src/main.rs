use rwikitext::{
    convert_with_options, ConversionError, ConversionMode, ConversionOptions, ConversionRequest,
    FileInfo, Format, LookupError, WikiContext, WikiData,
};
use serde_json::Value;
use std::io::{self, Read, Write};

struct LocalWiki(Value);

impl LocalWiki {
    fn load(path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let data: Value = serde_json::from_str(&std::fs::read_to_string(path)?)?;
        if !data.is_object() {
            return Err("wiki data must be a JSON object".into());
        }
        for section in ["context", "templates", "pages", "files"] {
            if data.get(section).is_some_and(|value| !value.is_object()) {
                return Err(format!("wiki data section {section} must be an object").into());
            }
        }
        for key in ["page_title", "site_language", "reference_time"] {
            if data["context"]
                .get(key)
                .is_some_and(|value| !value.is_string())
            {
                return Err(format!("wiki context field {key} must be a string").into());
            }
        }
        Ok(Self(data))
    }

    fn context(&self, key: &str) -> Option<&str> {
        self.0["context"][key].as_str()
    }

    fn entry(&self, section: &str, title: &str) -> Result<Option<&Value>, LookupError> {
        let Some(value) = self.0[section].get(title).filter(|value| !value.is_null()) else {
            return Ok(None);
        };
        if let Some(error) = value.get("error") {
            return Err(LookupError::new(error.as_str().ok_or_else(|| {
                LookupError::new(format!("{section} error must be a string"))
            })?));
        }
        Ok(Some(value))
    }
}

impl WikiData for LocalWiki {
    fn template_text(&self, title: &str) -> Result<Option<String>, LookupError> {
        self.entry("templates", title)?
            .map(|value| {
                value
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| LookupError::new("template text must be a string"))
            })
            .transpose()
    }

    fn page_exists(&self, title: &str) -> Result<Option<bool>, LookupError> {
        self.entry("pages", title)?
            .map(|value| {
                value
                    .as_bool()
                    .ok_or_else(|| LookupError::new("page existence must be a boolean"))
            })
            .transpose()
    }

    fn file_info(&self, title: &str) -> Result<Option<FileInfo>, LookupError> {
        self.entry("files", title)?
            .map(|value| {
                let url = value["url"]
                    .as_str()
                    .ok_or_else(|| LookupError::new("file URL must be a string"))?;
                let dimension = |key: &str| {
                    value
                        .get(key)
                        .filter(|value| !value.is_null())
                        .map(|value| {
                            value
                                .as_u64()
                                .and_then(|n| u32::try_from(n).ok())
                                .filter(|n| *n != 0)
                                .ok_or_else(|| {
                                    LookupError::new(format!(
                                        "file {key} must be a positive integer"
                                    ))
                                })
                        })
                        .transpose()
                };
                Ok(FileInfo {
                    url: url.into(),
                    width: dimension("width")?,
                    height: dimension("height")?,
                })
            })
            .transpose()
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut positional = Vec::new();
    let mut page_title = None;
    let mut site_language = None;
    let mut reference_time = None;
    let mut wiki_data_path = None;
    let mut mode = ConversionMode::BestEffort;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--strict" => mode = ConversionMode::Strict,
            "--wiki-data" => {
                wiki_data_path = Some(args.next().ok_or("missing value for --wiki-data")?)
            }
            "--page-title" => {
                page_title = Some(args.next().ok_or("missing value for --page-title")?)
            }
            "--site-language" => {
                site_language = Some(args.next().ok_or("missing value for --site-language")?)
            }
            "--reference-time" => {
                reference_time = Some(args.next().ok_or("missing value for --reference-time")?)
            }
            flag if flag.starts_with('-') && flag != "-" => {
                return Err(format!("unknown option: {flag}").into())
            }
            _ => positional.push(arg),
        }
    }
    if !(2..=3).contains(&positional.len()) {
        return Err("usage: rwikitext <wikitext|gfm|html> <wikitext|gfm|html> [FILE|-] [--wiki-data JSON] [--strict] [--page-title TITLE] [--site-language LANGUAGE] [--reference-time TIME]".into());
    }
    let source: Format = positional[0].parse()?;
    let target: Format = positional[1].parse()?;
    let mut input = String::new();
    match positional.get(2).map(String::as_str) {
        None | Some("-") => {
            io::stdin().read_to_string(&mut input)?;
        }
        Some(path) => input = std::fs::read_to_string(path)?,
    }
    let local = wiki_data_path.as_deref().map(LocalWiki::load).transpose()?;
    let defaults = WikiContext::default();
    let fixture = |key| local.as_ref().and_then(|data| data.context(key));
    let context = WikiContext {
        page_title: page_title
            .as_deref()
            .or_else(|| fixture("page_title"))
            .unwrap_or(defaults.page_title),
        site_language: site_language
            .as_deref()
            .or_else(|| fixture("site_language"))
            .unwrap_or(defaults.site_language),
        reference_time: reference_time
            .as_deref()
            .or_else(|| fixture("reference_time"))
            .unwrap_or(defaults.reference_time),
    };
    let result = convert_with_options(
        ConversionRequest {
            source,
            target,
            input: &input,
            context: &context,
        },
        ConversionOptions {
            mode,
            wiki_data: local.as_ref().map(|data| data as &dyn WikiData),
            ..ConversionOptions::default()
        },
    )?;
    for diagnostic in result.diagnostics {
        eprintln!("{diagnostic}");
    }
    io::stdout().write_all(result.content.as_bytes())?;
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        if let Some(error) = error
            .downcast_ref::<ConversionError>()
            .filter(|error| !error.diagnostics().is_empty())
        {
            for diagnostic in error.diagnostics() {
                eprintln!("{diagnostic}");
            }
        } else {
            eprintln!("rwikitext: {error}");
        }
        std::process::exit(1);
    }
}
