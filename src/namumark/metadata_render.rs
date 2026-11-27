use crate::{metadata, PageMetadata};

pub(crate) fn render_wikitext_metadata(metadata: &PageMetadata) -> String {
    let mut parts = Vec::new();
    if let Some(title) = &metadata.redirect_target {
        if !metadata::valid_title(title) {
            parts.push(crate::escape_wikitext(&format!("#redirect {title}")));
        } else {
            parts.push(format!("#REDIRECT [[{title}]]"));
        }
    }
    for (i, name) in metadata.categories.iter().enumerate() {
        let sort = metadata
            .category_sort_keys
            .get(i)
            .and_then(|s| s.as_deref());
        if !metadata::valid_title(name) {
            parts.push(crate::escape_wikitext(&format!(
                "[[분류:{name}{}]]",
                sort.map(|s| format!("|{s}")).unwrap_or_default()
            )));
        } else {
            parts.push(format!(
                "[[Category:{name}{}]]",
                sort.map(|s| format!("|{}", crate::escape_wikitext(s)))
                    .unwrap_or_default()
            ));
        }
    }
    parts.join("\n")
}
