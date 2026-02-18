pub fn base_url() -> String {
    "http://localhost:3000".to_string()
}

pub fn language() -> String {
    "en".to_string()
}

pub fn output_dir() -> String {
    "dist".to_string()
}

pub fn content_dir() -> String {
    "content".to_string()
}

pub fn template_dir() -> String {
    "templates".to_string()
}

pub fn static_dir() -> String {
    "static".to_string()
}

pub fn data_dir() -> String {
    "data".to_string()
}

pub fn image_widths() -> Vec<u32> {
    vec![480, 800, 1200]
}

pub fn image_quality() -> u8 {
    80
}

pub fn bool_true() -> bool {
    true
}
