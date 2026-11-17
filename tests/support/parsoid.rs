use rwikitext::WikiContext;
use std::path::Path;
use std::process::Command;

pub fn parsoid_command(context: &WikiContext<'_>) -> Command {
    let php = std::env::var_os("RWIKITEXT_PHP").unwrap_or_else(|| "php".into());
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut command = Command::new(php);
    command
        .arg(root.join("tests/render-parsoid.php"))
        .arg(root.join("target/upstream/mediawiki-1.46.2"))
        .args([
            context.page_title,
            context.site_language,
            context.reference_time,
        ]);
    command
}
