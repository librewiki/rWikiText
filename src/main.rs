use rwikitext::{
    convert_with_mode, ConversionError, ConversionMode, ConversionRequest, Format, WikiContext,
};
use std::io::{self, Read, Write};

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut positional = Vec::new();
    let mut page_title = String::new();
    let mut site_language = String::from("en");
    let mut reference_time = String::from("1970-01-01T00:00:00Z");
    let mut mode = ConversionMode::BestEffort;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--strict" => mode = ConversionMode::Strict,
            "--page-title" => page_title = args.next().ok_or("missing value for --page-title")?,
            "--site-language" => {
                site_language = args.next().ok_or("missing value for --site-language")?
            }
            "--reference-time" => {
                reference_time = args.next().ok_or("missing value for --reference-time")?
            }
            flag if flag.starts_with('-') && flag != "-" => {
                return Err(format!("unknown option: {flag}").into())
            }
            _ => positional.push(arg),
        }
    }
    if !(2..=3).contains(&positional.len()) {
        return Err("usage: rwikitext <wikitext|gfm|html> <wikitext|gfm|html> [FILE|-] [--strict] [--page-title TITLE] [--site-language LANGUAGE] [--reference-time TIME]".into());
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
    let context = WikiContext {
        page_title: &page_title,
        site_language: &site_language,
        reference_time: &reference_time,
    };
    let result = convert_with_mode(
        ConversionRequest {
            source,
            target,
            input: &input,
            context: &context,
        },
        mode,
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
