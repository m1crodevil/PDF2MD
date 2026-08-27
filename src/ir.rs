use serde::{Deserialize, Serialize};

// The IR is introduced before backend wiring; fields are consumed by the next
// phase. Keep the schema typed now so routing cannot invent ad-hoc JSON later.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) enum Backend {
    PdfOxide,
    Pdfium,
    Ocr,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)]
pub(crate) struct PageProbe {
    pub page: usize,
    pub native_text_chars: usize,
    pub page_area: f64,
    pub image_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[allow(dead_code)]
pub(crate) struct PageIr {
    pub schema: String,
    pub page: usize,
    pub backend: Backend,
    pub text: String,
    pub width: f64,
    pub height: f64,
}

impl PageProbe {
    #[allow(dead_code)]
    pub(crate) fn backend(&self, min_text_chars: usize) -> Backend {
        if self.image_only || self.native_text_chars < min_text_chars {
            Backend::Ocr
        } else {
            Backend::PdfOxide
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Backend, PageProbe};

    #[test]
    fn routes_text_pages_to_pdfoxide_and_sparse_pages_to_ocr() {
        let text_page = PageProbe {
            page: 1,
            native_text_chars: 120,
            page_area: 1.0,
            image_only: false,
        };
        let sparse_page = PageProbe {
            page: 2,
            native_text_chars: 3,
            page_area: 1.0,
            image_only: false,
        };
        let image_page = PageProbe {
            page: 3,
            native_text_chars: 800,
            page_area: 1.0,
            image_only: true,
        };
        assert_eq!(text_page.backend(20), Backend::PdfOxide);
        assert_eq!(sparse_page.backend(20), Backend::Ocr);
        assert_eq!(image_page.backend(20), Backend::Ocr);
    }
}
