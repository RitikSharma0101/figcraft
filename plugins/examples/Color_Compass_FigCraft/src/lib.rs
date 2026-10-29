
use figcraft_plugin_sdk::{
    Event, Input, Result, Value, call, entry, json, notify, set_setting, setting, show_ui,
};

const HARMONIES: &[&str] = &[
    "Complementary",
    "Analogous",
    "Triadic",
    "Split complementary",
    "Tetradic",
    "Shades",
    "Tints",
    "Saturate",
    "Desaturate",
];

fn clean_hex(s: &str) -> Option<(u8, u8, u8)> {
    let h = s.trim().trim_start_matches('#');
    if h.len() != 6 || !h.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    Some((
        u8::from_str_radix(&h[0..2], 16).ok()?,
        u8::from_str_radix(&h[2..4], 16).ok()?,
        u8::from_str_radix(&h[4..6], 16).ok()?,
    ))
}

fn hex_rgb(r: f64, g: f64, b: f64) -> String {
    format!(
        "#{:02X}{:02X}{:02X}",
        r.round().clamp(0.0, 255.0) as u8,
        g.round().clamp(0.0, 255.0) as u8,
        b.round().clamp(0.0, 255.0) as u8
    )
}

fn rgb_hsl(r: u8, g: u8, b: u8) -> (f64, f64, f64) {
    let (r, g, b) = (r as f64 / 255.0, g as f64 / 255.0, b as f64 / 255.0);
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let d = max - min;
    let l = (max + min) / 2.0;

    if d == 0.0 {
        return (0.0, 0.0, l);
    }

    let s = d / (1.0 - (2.0 * l - 1.0).abs());
    let mut h = if max == r {
        ((g - b) / d) % 6.0
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };

    h *= 60.0;
    if h < 0.0 {
        h += 360.0;
    }
    (h, s, l)
}

fn hsl_rgb(h: f64, s: f64, l: f64) -> String {
    let h = h.rem_euclid(360.0);
    let s = s.clamp(0.0, 1.0);
    let l = l.clamp(0.0, 1.0);
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = l - c / 2.0;

    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    hex_rgb((r + m) * 255.0, (g + m) * 255.0, (b + m) * 255.0)
}

fn make_palette(hex: &str, harmony: &str, steps: usize) -> Result<Vec<String>> {
    let (r, g, b) = clean_hex(hex)
        .ok_or_else(|| "Invalid HEX colour. Example: #3878E8".to_string())?;

    let steps = steps.clamp(3, 12);
    let (h, s, l) = rgb_hsl(r, g, b);

    let colors = match harmony {
        "Shades" => (0..steps)
            .map(|i| hsl_rgb(h, s, l * (1.0 - i as f64 / steps as f64)))
            .collect(),

        "Tints" => (0..steps)
            .map(|i| {
                let amount = (i + 1) as f64 / steps as f64;
                hsl_rgb(h, s, l + (1.0 - l) * amount)
            })
            .collect(),

        "Saturate" => (0..steps)
            .map(|i| {
                let amount = (i + 1) as f64 / steps as f64;
                hsl_rgb(h, s + (1.0 - s) * amount, l)
            })
            .collect(),

        "Desaturate" => (0..steps)
            .map(|i| {
                let amount = (i + 1) as f64 / steps as f64;
                hsl_rgb(h, s * (1.0 - amount), l)
            })
            .collect(),

        _ => {
            let angles: Vec<f64> = match harmony {
                "Analogous" => vec![-40.0, -20.0, 0.0, 20.0, 40.0],
                "Triadic" => vec![0.0, 120.0, 240.0],
                "Split complementary" => vec![0.0, 150.0, 210.0],
                "Tetradic" => vec![0.0, 90.0, 180.0, 270.0],
                _ => vec![0.0, 180.0],
            };

            (0..steps)
                .map(|i| {
                    let angle = angles[i % angles.len()];
                    hsl_rgb(h + angle, s, l)
                })
                .collect()
        }
    };

    Ok(colors)
}

fn draw_swatches(colors: &[String]) -> Result<Value> {
    if colors.is_empty() {
        return Err("No colours to add.".into());
    }

    let columns = 5usize;
    let rows = colors.len().div_ceil(columns);
    let width = columns as f64 * 112.0 - 12.0;
    let height = rows as f64 * 92.0 - 12.0;

    let view = call("view.get", &json!({}))?;
    let x0 = (view["cx"].as_f64().unwrap_or(0.0) - width / 2.0).round();
    let y0 = (view["cy"].as_f64().unwrap_or(0.0) - height / 2.0).round();
    let mut ids: Vec<u64> = Vec::new();

    for (i, color) in colors.iter().enumerate() {
        let x = x0 + (i % columns) as f64 * 112.0;
        let y = y0 + (i / columns) as f64 * 92.0;

        let rect = call(
            "shape.rectangle",
            &json!({
                "x": x,
                "y": y,
                "width": 100.0,
                "height": 58.0,
                "props": {
                    "name": format!("Color {}", color),
                    "fill": color
                }
            }),
        )?;

        if let Some(id) = rect["id"].as_u64() {
            ids.push(id);
        }

        let label = call(
            "text.create",
            &json!({
                "x": x,
                "y": y + 62.0,
                "text": color,
                "props": {
                    "fontSize": 12,
                    "fill": "#202124"
                }
            }),
        )?;

        if let Some(id) = label["id"].as_u64() {
            ids.push(id);
        }
    }

    call("select.set", &json!({"ids": ids}))?;
    notify(&format!("Added {} colour swatches", colors.len()));

    Ok(json!({"swatches": colors.len(), "ids": ids}))
}

fn panel(hex: &str, harmony: &str, steps: usize, preview: &str) -> Result<Value> {
    show_ui(&json!({
        "title": "Color Compass",
        "controls": [
            {"type": "note", "label": "COLOR COMPASS — Colour harmony studio"},
            {"type": "color", "name": "hex", "label": "Base colour", "value": hex},
            {
                "type": "list",
                "name": "harmony",
                "label": "Colour mode",
                "options": HARMONIES,
                "value": harmony
            },
            {"type": "number", "name": "steps", "label": "Number of colours",
             "min": 3, "max": 12, "value": steps},
            {"type": "note", "label": preview},
            {"type": "button", "name": "generate", "label": "Generate palette"},
            {"type": "button", "name": "canvas", "label": "Add palette to canvas"}
        ]
    }))?;
    Ok(Value::Null)
}

fn on_event(event: Event) -> Result<Value> {
    let hex = event.values["hex"]
        .as_str()
        .unwrap_or("#3878E8")
        .to_string();

    let harmony = event.values["harmony"]
        .as_str()
        .unwrap_or("Complementary")
        .to_string();

    let steps = event.values["steps"]
        .as_f64()
        .unwrap_or(5.0)
        .round()
        .clamp(3.0, 12.0) as usize;

    if event.control != "generate" && event.control != "canvas" {
        return panel(&hex, &harmony, steps, "Settings updated. Generate to preview.");
    }

    set_setting("base_hex", &json!(hex))?;
    let colors = make_palette(&hex, &harmony, steps)?;

    if event.control == "canvas" {
        return draw_swatches(&colors);
    }

    let preview = colors.join("   ");
    panel(&hex, &harmony, steps, &preview)?;
    notify("Palette generated.");
    Ok(json!({"base": hex, "harmony": harmony, "colors": colors}))
}

fn run(input: Input) -> Result<Value> {
    if let Some(event) = input.event {
        return on_event(event);
    }

    if input.command == "panel" {
        let hex = setting("base_hex")
            .and_then(|v| v.as_str().map(str::to_string))
            .unwrap_or_else(|| "#3878E8".to_string());

        return panel(&hex, "Complementary", 5, "Choose a colour mode and generate.");
    }

    let hex = input.str("hex").unwrap_or("#3878E8");
    let harmony = input.str("harmony").unwrap_or("Complementary");
    let steps = input.f64("steps").unwrap_or(5.0)
        .round().clamp(3.0, 12.0) as usize;

    let colors = make_palette(hex, harmony, steps)?;
    draw_swatches(&colors)
}

entry!(run);
 