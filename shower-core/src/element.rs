use std::slice;

const TINY_SMALL_F64: f64 = 0.00000001;

#[derive(Debug, Clone)]
pub(crate) enum Element {
    Long(i64),
    Double(f64),
    Str(u64, usize, usize),
}
impl Element {
    pub(crate) fn len(&self) -> usize {
        match self {
            Element::Long(_) => 8,
            Element::Double(_) => 8,
            Element::Str(_, _, len) => *len,
        }
    }
    pub(crate) fn copy_to_target(&self, target: &mut [u8]) {
        match self {
            Element::Long(val) => target.copy_from_slice(val.to_ne_bytes().as_slice()),
            Element::Double(val) => target.copy_from_slice(val.to_ne_bytes().as_slice()),
            Element::Str(u64ptr, offset, len) => {
                let ptr_slice = (*u64ptr + *offset as u64) as *const u8;
                let slice = unsafe { slice::from_raw_parts(ptr_slice, *len) };
                target.copy_from_slice(slice)
            }
        };
    }
    pub(crate) fn _is_computed(&self) -> bool {
        match self {
            Element::Long(_) => true,
            Element::Double(_) => true,
            Element::Str(_, _, _) => false,
        }
    }
    pub(crate) fn add(self, other: Element) -> Element {
        match (self, other) {
            (Element::Long(v1), Element::Long(v2)) => Element::Long(v1 + v2),
            (Element::Long(v1), Element::Double(v2)) => Element::Double(v1 as f64 + v2),
            (Element::Double(v1), Element::Long(v2)) => Element::Double(v1 + v2 as f64),
            (Element::Double(v1), Element::Double(v2)) => Element::Double(v1 + v2),
            _ => Element::Long(0),
        }
    }
    pub(crate) fn sub(self, other: Element) -> Element {
        match (self, other) {
            (Element::Long(v1), Element::Long(v2)) => Element::Long(v1 - v2),
            (Element::Long(v1), Element::Double(v2)) => Element::Double(v1 as f64 - v2),
            (Element::Double(v1), Element::Long(v2)) => Element::Double(v1 - v2 as f64),
            (Element::Double(v1), Element::Double(v2)) => Element::Double(v1 - v2),
            _ => Element::Long(0),
        }
    }
    pub(crate) fn mul(self, other: Element) -> Element {
        match (self, other) {
            (Element::Long(v1), Element::Long(v2)) => Element::Long(v1 * v2),
            (Element::Long(v1), Element::Double(v2)) => Element::Double(v1 as f64 * v2),
            (Element::Double(v1), Element::Long(v2)) => Element::Double(v1 * v2 as f64),
            (Element::Double(v1), Element::Double(v2)) => Element::Double(v1 * v2),
            _ => Element::Long(0),
        }
    }
    pub(crate) fn div(self, other: Element) -> Element {
        match (self, other) {
            (Element::Long(v1), Element::Long(v2)) => Element::Long(v1 / v2),
            (Element::Long(v1), Element::Double(v2)) => Element::Double(v1 as f64 / v2),
            (Element::Double(v1), Element::Long(v2)) => Element::Double(v1 / v2 as f64),
            (Element::Double(v1), Element::Double(v2)) => Element::Double(v1 / v2),
            _ => Element::Long(0),
        }
    }
    pub(crate) fn mod_(self, other: Element) -> Element {
        match (self, other) {
            (Element::Long(v1), Element::Long(v2)) => Element::Long(v1 % v2),
            _ => Element::Long(0),
        }
    }
    pub(crate) fn eq(self, expacted: Element) -> bool {
        match (self, expacted) {
            (Element::Long(v1), Element::Long(v2)) => v1 == v2,
            (Element::Double(v1), Element::Double(v2)) => f64::abs(v1 - v2) < TINY_SMALL_F64,
            _ => false,
        }
    }
    pub(crate) fn gt_eq(self, expacted: Element) -> bool {
        match (self, expacted) {
            (Element::Long(v1), Element::Long(v2)) => v1 >= v2,
            (Element::Double(v1), Element::Double(v2)) => {
                v1 > v2 || f64::abs(v1 - v2) < TINY_SMALL_F64
            }
            _ => false,
        }
    }
    pub(crate) fn gt(self, expacted: Element) -> bool {
        match (self, expacted) {
            (Element::Long(v1), Element::Long(v2)) => v1 > v2,
            (Element::Double(v1), Element::Double(v2)) => v1 > v2,
            _ => false,
        }
    }
    pub(crate) fn lt_eq(self, expacted: Element) -> bool {
        match (self, expacted) {
            (Element::Long(v1), Element::Long(v2)) => v1 <= v2,
            (Element::Double(v1), Element::Double(v2)) => {
                v1 < v2 || f64::abs(v1 - v2) < TINY_SMALL_F64
            }
            _ => false,
        }
    }
    pub(crate) fn lt(self, expacted: Element) -> bool {
        match (self, expacted) {
            (Element::Long(v1), Element::Long(v2)) => v1 < v2,
            (Element::Double(v1), Element::Double(v2)) => v1 > v2,
            _ => false,
        }
    }
    pub(crate) fn neq(self, expacted: Element) -> bool {
        match (self, expacted) {
            (Element::Long(v1), Element::Long(v2)) => v1 != v2,
            (Element::Double(v1), Element::Double(v2)) => f64::abs(v1 - v2) > TINY_SMALL_F64,
            _ => true,
        }
    }
}
