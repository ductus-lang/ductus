use kurbo::{BezPath, DEFAULT_ACCURACY, Point, Shape};

pub struct SvgGenerator;

impl SvgGenerator {
    pub fn generate_svg(drawn_paths: &[BezPath], point_labels: &[(Point, String)]) -> String {
        let mut overall_bbox = None;
        let has_debug = !point_labels.is_empty();
        if !has_debug {
            for path in drawn_paths {
                let bbox = path.bounding_box();
                match overall_bbox {
                    None => overall_bbox = Some(bbox),
                    Some(ref mut current_bbox) => {
                        *current_bbox = current_bbox.union(bbox);
                    }
                }
            }
        }
        let (viewbox_str, width, height) = match overall_bbox {
            Some(bbox) => {
                let margin = if point_labels.is_empty() { 10.0 } else { 50.0 }; // Increased margin for labels
                let x0 = bbox.x0 - margin;
                let y0 = bbox.y0 - margin;
                let width = bbox.width() + 2.0 * margin;
                let height = bbox.height() + 2.0 * margin;
                (
                    format!("{} {} {} {}", x0, y0, width, height),
                    width as u32,
                    height as u32,
                )
            }
            None => ("0 0 1920 1080".to_string(), 1920, 1080),
        };

        let mut svg = format!(
            r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{}" width="{}" height="{}">"#,
            viewbox_str, width, height
        );

        svg.push('\n');

        svg.push_str(
            r#"
<defs>
    <style>
       <![CDATA[
       .chi-path {
            fill: #f9e2af;
            stroke: #a6e3a1;
            stroke-width: 3;
        }
    ]]>
    </style>
</defs>
"#,
        );
        // Add CSS styles if in debug mode
        if has_debug {
            svg.push_str(
                r#"
<defs>
    <style>
        <![CDATA[
        .debug-point {
            fill: #f38ba8;
            stroke: #cdd6f4;
            stroke-width: 0.5;
        }
        .debug-text {
            font-family: monospace;
            font-size: 14px;
            fill:#fab387;
            stroke:  #313244;
            stroke-width: 0.3;
            paint-order: stroke fill;
        }
        .grid {
            fill: url(#grid);
            fill-opacity: 0.9;
        }
        path {
            stroke:#f5e0dc;
        }
        .debug-background {
           fill: #313244; 
        }
     ]]>
    </style>
<pattern id="smallGrid" width="10" height="10" patternUnits="userSpaceOnUse">
<path d="M 10 0 L 0 0 0 10" fill="none" stroke="gray" stroke-width="0.3" stroke-opacity="0.8"/>
</pattern>
<pattern id="mediumGrid" width="50" height="50" patternUnits="userSpaceOnUse">
<rect width="50" height="50" fill="url(#smallGrid)"/>
<path d="M 50 0 L 0 0 0 50" fill="none" stroke="gray" stroke-width="0.6" stroke-opacity="0.5"/>
</pattern>

<pattern id="grid" width="100" height="100" patternUnits="userSpaceOnUse">
<rect width="100" height="100" fill="url(#mediumGrid)"/>
<path d="M 100 0 L 0 0 0 100" fill="none" stroke="gray" stroke-width="1" stroke-opacity="0.3"/>
</pattern>
</defs>
"#,
            );

            let parts: Vec<&str> = viewbox_str.split_whitespace().collect();
            if parts.len() == 4 {
                svg.push_str(&format!(
                    r#" 
                    <rect class="debug-background" x="{}" y="{}" width="{}" height="{}"/>
                    <rect class="grid" x="{}" y="{}" width="{}" height="{}"/>"#,
                    parts[0], parts[1], parts[2], parts[3], parts[0], parts[1], parts[2], parts[3]
                ));
                svg.push('\n');
            }
        }

        // Add paths
        for path in drawn_paths {
            svg.push_str(&format!(
                r#"  <path class="chi-path" d="{}"/>"#,
                path.to_path(DEFAULT_ACCURACY).to_svg()
            ));
            svg.push('\n');
        }

        // Add point labels if in debug mode
        for (point, label) in point_labels {
            // Add a small circle for the point
            svg.push_str(&format!(
                r##"  <circle class="debug-point" cx="{}" cy="{}" r="5"/>"##,
                point.x, point.y
            ));
            svg.push('\n');

            // Add the text label offset slightly from the point
            svg.push_str(&format!(
                r##"  <text class="debug-text" x="{}" y="{}">{}</text>"##,
                point.x + 8.0,
                point.y - 8.0,
                label
            ));
            svg.push('\n');
        }

        svg.push_str("</svg>");
        svg
    }
}
