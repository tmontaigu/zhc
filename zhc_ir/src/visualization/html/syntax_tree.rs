use crate::visualization::svg::Svg;

const HTML_TEMPLATE: &str = include_str!("template.html");

/// An HTML document wrapping an SVG visualization.
#[derive(Debug, Clone)]
pub struct Html {
    /// Title shown in the browser tab
    pub title: String,
    /// The SVG content to embed
    pub svg: Svg,
    /// Additional CSS for the HTML wrapper
    pub css: String,
    /// JavaScript for zoom/pan and interactions
    pub javascript: String,
}

impl std::fmt::Display for Html {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.render_document())
    }
}

impl Html {
    /// Renders the full HTML document from the static template.
    fn render_document(&self) -> String {
        let title = html_escape(&self.title);
        HTML_TEMPLATE
            .replace("{{TITLE}}", &title)
            .replace("{{CSS}}", &self.css)
            .replace("{{SVG}}", &self.svg_with_viewbox())
            .replace("{{JAVASCRIPT}}", &self.javascript)
    }

    /// Renders the SVG with viewBox for proper scaling.
    fn svg_with_viewbox(&self) -> String {
        let svg = &self.svg;
        let mut output = String::new();

        // SVG opening tag with viewBox
        output.push_str(&format!(
            r#"<svg viewBox="0 0 {} {}" preserveAspectRatio="xMidYMid meet" xmlns="http://www.w3.org/2000/svg">"#,
            svg.width, svg.height
        ));
        output.push('\n');

        // Elements
        for element in &svg.elements {
            output.push_str(&format!("{}", element));
        }

        output.push_str("</svg>\n");
        output
    }
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}
