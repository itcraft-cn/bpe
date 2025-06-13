use crate::aux::fill_ptr;

#[derive(Debug, Clone)]
pub(crate) enum Element {
    Long(i64),
    Double(f64),
}
impl Element {
    pub(crate) fn len(&self) -> usize {
        match self {
            Element::Long(_) => 8,
            Element::Double(_) => 8,
        }
    }
    pub(crate) fn copy_to_target(&self, u8_ptr: *mut u8, offset: usize) {
        let adjusted = unsafe { u8_ptr.add(offset) };
        match self {
            Element::Long(val) => fill_ptr(adjusted, *val),
            Element::Double(val) => fill_ptr(adjusted, *val),
        };
    }
    pub(crate) fn _is_computed(&self) -> bool {
        match self {
            Element::Long(_) => true,
            Element::Double(_) => true,
        }
    }
    pub(crate) fn add(self, other: Element) -> Element {
        match (self, other) {
            (Element::Long(v1), Element::Long(v2)) => Element::Long(v1 + v2),
            (Element::Long(v1), Element::Double(v2)) => Element::Double(v1 as f64 + v2),
            (Element::Double(v1), Element::Long(v2)) => Element::Double(v1 + v2 as f64),
            (Element::Double(v1), Element::Double(v2)) => Element::Double(v1 + v2),
        }
    }
    pub(crate) fn sub(self, other: Element) -> Element {
        match (self, other) {
            (Element::Long(v1), Element::Long(v2)) => Element::Long(v1 - v2),
            (Element::Long(v1), Element::Double(v2)) => Element::Double(v1 as f64 - v2),
            (Element::Double(v1), Element::Long(v2)) => Element::Double(v1 - v2 as f64),
            (Element::Double(v1), Element::Double(v2)) => Element::Double(v1 - v2),
        }
    }
    pub(crate) fn mul(self, other: Element) -> Element {
        match (self, other) {
            (Element::Long(v1), Element::Long(v2)) => Element::Long(v1 * v2),
            (Element::Long(v1), Element::Double(v2)) => Element::Double(v1 as f64 * v2),
            (Element::Double(v1), Element::Long(v2)) => Element::Double(v1 * v2 as f64),
            (Element::Double(v1), Element::Double(v2)) => Element::Double(v1 * v2),
        }
    }
    pub(crate) fn div(self, other: Element) -> Element {
        match (self, other) {
            (Element::Long(v1), Element::Long(v2)) => Element::Long(v1 / v2),
            (Element::Long(v1), Element::Double(v2)) => Element::Double(v1 as f64 / v2),
            (Element::Double(v1), Element::Long(v2)) => Element::Double(v1 / v2 as f64),
            (Element::Double(v1), Element::Double(v2)) => Element::Double(v1 / v2),
        }
    }
    pub(crate) fn mod_(self, other: Element) -> Element {
        match (self, other) {
            (Element::Long(v1), Element::Long(v2)) => Element::Long(v1 % v2),
            _ => Element::Long(0),
        }
    }
}
