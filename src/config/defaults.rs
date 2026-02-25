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

pub fn public_dir() -> String {
    "public".to_string()
}

pub fn image_widths() -> Vec<u32> {
    vec![480, 800, 1200]
}

pub fn image_quality() -> u8 {
    80
}

pub fn avif_quality() -> u8 {
    70
}

pub fn bool_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_url() {
        assert_eq!(base_url(), "http://localhost:3000");
    }

    #[test]
    fn test_language() {
        assert_eq!(language(), "en");
    }

    #[test]
    fn test_output_dir() {
        assert_eq!(output_dir(), "dist");
    }

    #[test]
    fn test_content_dir() {
        assert_eq!(content_dir(), "content");
    }

    #[test]
    fn test_template_dir() {
        assert_eq!(template_dir(), "templates");
    }

    #[test]
    fn test_static_dir() {
        assert_eq!(static_dir(), "static");
    }

    #[test]
    fn test_data_dir() {
        assert_eq!(data_dir(), "data");
    }

    #[test]
    fn test_public_dir() {
        assert_eq!(public_dir(), "public");
    }

    #[test]
    fn test_image_widths() {
        assert_eq!(image_widths(), vec![480, 800, 1200]);
    }

    #[test]
    fn test_image_quality() {
        assert_eq!(image_quality(), 80);
    }

    #[test]
    fn test_avif_quality() {
        assert_eq!(avif_quality(), 70);
    }

    #[test]
    fn test_bool_true() {
        assert!(bool_true());
    }
}
