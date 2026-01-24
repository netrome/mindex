//! Math expression rendering via LaTeX to MathML conversion.
//!
//! This module provides an abstraction layer for converting LaTeX math expressions
//! to MathML, allowing the underlying implementation to be swapped if needed.

/// Style of math display.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MathStyle {
    /// Inline math, rendered within text flow (e.g., `$x^2$`).
    Inline,
    /// Display math, rendered as a centered block (e.g., `$$\int_0^1 x dx$$`).
    Display,
}

/// Result of a math rendering operation.
#[derive(Debug, Clone)]
pub(crate) enum MathResult {
    /// Successfully rendered MathML.
    MathMl(String),
    /// Failed to render; contains fallback HTML showing the raw LaTeX.
    Fallback(String),
}

impl MathResult {
    /// Convert the result to an HTML string.
    pub(crate) fn into_html(self) -> String {
        match self {
            MathResult::MathMl(html) => html,
            MathResult::Fallback(html) => html,
        }
    }
}

/// Render a LaTeX math expression to MathML HTML.
///
/// On success, returns MathML markup that browsers can render natively.
/// On failure, returns a fallback `<code>` block showing the raw LaTeX.
pub(crate) fn render_math(latex: &str, style: MathStyle) -> MathResult {
    let display_style = match style {
        MathStyle::Inline => latex2mathml::DisplayStyle::Inline,
        MathStyle::Display => latex2mathml::DisplayStyle::Block,
    };

    match latex2mathml::latex_to_mathml(latex, display_style) {
        Ok(mathml) => MathResult::MathMl(mathml),
        Err(_) => {
            let escaped = html_escape(latex);
            let fallback = match style {
                MathStyle::Inline => {
                    format!(
                        r#"<code class="math-error" title="Failed to render math">{escaped}</code>"#
                    )
                }
                MathStyle::Display => {
                    format!(
                        r#"<pre class="math-error" title="Failed to render math"><code>{escaped}</code></pre>"#
                    )
                }
            };
            MathResult::Fallback(fallback)
        }
    }
}

/// Escape HTML special characters in a string.
pub(crate) fn html_escape(s: &str) -> String {
    let mut escaped = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#x27;"),
            _ => escaped.push(c),
        }
    }
    escaped
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    #[test]
    fn render_math__should_render_simple_inline_math() {
        // Given
        let latex = "x^2";

        // When
        let result = render_math(latex, MathStyle::Inline);

        // Then
        match result {
            MathResult::MathMl(mathml) => {
                assert!(mathml.contains("<math"));
                assert!(mathml.contains("</math>"));
            }
            MathResult::Fallback(_) => panic!("Expected MathMl, got Fallback"),
        }
    }

    #[test]
    fn render_math__should_render_simple_display_math() {
        // Given
        let latex = r"\int_0^1 x \, dx";

        // When
        let result = render_math(latex, MathStyle::Display);

        // Then
        match result {
            MathResult::MathMl(mathml) => {
                assert!(mathml.contains("<math"));
                assert!(mathml.contains("</math>"));
                // Display style should have display="block"
                assert!(mathml.contains(r#"display="block""#));
            }
            MathResult::Fallback(_) => panic!("Expected MathMl, got Fallback"),
        }
    }

    #[test]
    fn render_math__should_render_fractions() {
        // Given
        let latex = r"\frac{a}{b}";

        // When
        let result = render_math(latex, MathStyle::Inline);

        // Then
        match result {
            MathResult::MathMl(mathml) => {
                assert!(mathml.contains("<mfrac>"));
            }
            MathResult::Fallback(_) => panic!("Expected MathMl, got Fallback"),
        }
    }

    #[test]
    fn render_math__should_fallback_on_invalid_latex() {
        // Given - unclosed brace
        let latex = r"\frac{a}{";

        // When
        let result = render_math(latex, MathStyle::Inline);

        // Then
        match result {
            MathResult::Fallback(html) => {
                assert!(html.contains("math-error"));
                assert!(html.contains(r"\frac{a}{"));
            }
            MathResult::MathMl(_) => panic!("Expected Fallback, got MathMl"),
        }
    }

    #[test]
    fn render_math__should_escape_html_in_fallback() {
        // Given - invalid LaTeX with HTML special chars
        let latex = r"<script>alert('xss')</script>";

        // When
        let result = render_math(latex, MathStyle::Inline);

        // Then
        match result {
            MathResult::Fallback(html) => {
                assert!(!html.contains("<script>"));
                assert!(html.contains("&lt;script&gt;"));
            }
            MathResult::MathMl(_) => {
                // If it somehow renders, that's fine too
            }
        }
    }

    #[test]
    fn render_math__display_fallback_should_use_pre() {
        // Given
        let latex = r"\invalid{";

        // When
        let result = render_math(latex, MathStyle::Display);

        // Then
        match result {
            MathResult::Fallback(html) => {
                assert!(html.contains("<pre"));
                assert!(html.contains("</pre>"));
            }
            MathResult::MathMl(_) => panic!("Expected Fallback, got MathMl"),
        }
    }

    #[test]
    fn html_escape__should_escape_special_characters() {
        assert_eq!(html_escape("&"), "&amp;");
        assert_eq!(html_escape("<"), "&lt;");
        assert_eq!(html_escape(">"), "&gt;");
        assert_eq!(html_escape("\""), "&quot;");
        assert_eq!(html_escape("'"), "&#x27;");
        assert_eq!(html_escape("a < b & c > d"), "a &lt; b &amp; c &gt; d");
    }

    #[test]
    fn math_result_into_html__should_return_content() {
        let mathml = MathResult::MathMl("<math>x</math>".to_string());
        assert_eq!(mathml.into_html(), "<math>x</math>");

        let fallback = MathResult::Fallback("<code>x</code>".to_string());
        assert_eq!(fallback.into_html(), "<code>x</code>");
    }
}
