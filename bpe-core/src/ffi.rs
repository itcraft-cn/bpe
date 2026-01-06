use crate::fndef::CallbackParams;

pub trait FfiFunc {
    fn callback(&self, param: CallbackParams);
}
