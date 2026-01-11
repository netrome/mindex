use askama::Template;
use askama_web::WebTemplate;

#[derive(Template, WebTemplate)]
#[template(path = "document_list.html")]
pub(crate) struct DocumentListTemplate {
    pub(crate) app_name: String,
    pub(crate) documents: Vec<String>,
    pub(crate) git_enabled: bool,
}

#[derive(Template, WebTemplate)]
#[template(path = "new.html")]
pub(crate) struct NewDocumentTemplate {
    pub(crate) app_name: String,
    pub(crate) doc_id: String,
    pub(crate) error: String,
    pub(crate) git_enabled: bool,
}

#[derive(Template)]
#[template(path = "manifest.json", escape = "none")]
pub(crate) struct ManifestTemplate<'a> {
    pub(crate) app_name: &'a str,
}

#[derive(Template)]
#[template(path = "sw.js", escape = "none")]
pub(crate) struct ServiceWorkerTemplate {
    pub(crate) auth_enabled: bool,
}

#[derive(Template, WebTemplate)]
#[template(path = "document.html")]
pub(crate) struct DocumentTemplate {
    pub(crate) app_name: String,
    pub(crate) doc_id: String,
    pub(crate) content: String,
    pub(crate) git_enabled: bool,
}

#[derive(Template, WebTemplate)]
#[template(path = "edit.html")]
pub(crate) struct EditTemplate {
    pub(crate) app_name: String,
    pub(crate) doc_id: String,
    pub(crate) contents: String,
    pub(crate) notice: String,
    pub(crate) git_enabled: bool,
}

mod filters {
    use std::fmt::Write;

    pub fn json_escape(value: &str, _values: &dyn askama::Values) -> askama::Result<String> {
        let mut escaped = String::with_capacity(value.len());
        for ch in value.chars() {
            match ch {
                '"' => escaped.push_str("\\\""),
                '\\' => escaped.push_str("\\\\"),
                '\n' => escaped.push_str("\\n"),
                '\r' => escaped.push_str("\\r"),
                '\t' => escaped.push_str("\\t"),
                '\u{08}' => escaped.push_str("\\b"),
                '\u{0C}' => escaped.push_str("\\f"),
                ch if ch < '\u{20}' => {
                    write!(escaped, "\\u{:04x}", ch as u32)?;
                }
                _ => escaped.push(ch),
            }
        }
        Ok(escaped)
    }
}

#[derive(Template, WebTemplate)]
#[template(path = "search.html")]
pub(crate) struct SearchTemplate {
    pub(crate) app_name: String,
    pub(crate) query: String,
    pub(crate) results: Vec<SearchResult>,
    pub(crate) git_enabled: bool,
}

pub(crate) struct SearchResult {
    pub(crate) doc_id: String,
    pub(crate) snippet: String,
}

#[derive(Template, WebTemplate)]
#[template(path = "push_subscribe.html")]
pub(crate) struct PushSubscribeTemplate {
    pub(crate) app_name: String,
    pub(crate) git_enabled: bool,
}

#[derive(Template, WebTemplate)]
#[template(path = "login.html")]
pub(crate) struct LoginTemplate {
    pub(crate) app_name: String,
    pub(crate) error: String,
    pub(crate) next: String,
    pub(crate) git_enabled: bool,
}

#[derive(Template, WebTemplate)]
#[template(path = "git.html")]
pub(crate) struct GitTemplate {
    pub(crate) app_name: String,
    pub(crate) status: String,
    pub(crate) diff: String,
    pub(crate) message: String,
    pub(crate) error: String,
    pub(crate) notice: String,
    pub(crate) git_enabled: bool,
}
