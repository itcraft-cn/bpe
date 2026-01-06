use crate::FfiFunc;

pub(crate) type NormalFunc = Box<dyn Fn(CallbackParams) + Send + 'static>;
pub(crate) type LambdaFunc = Box<dyn Fn(CallbackParams) + 'static>;

pub(crate) enum FnHolder {
    Func(NormalFunc),
    FfiFunc(Box<dyn FfiFunc>),
    Lambda(LambdaFunc),
}

pub struct CallbackParams {
    u8_ptr: *const u8,
    mask: usize,
    offset: usize,
    size: usize,
    step: usize,
}
impl CallbackParams {
    pub fn new(
        u8_ptr: *const u8,
        mask: usize,
        offset: usize,
        size: usize,
        step: usize,
    ) -> Self {
        Self {
            u8_ptr,
            mask,
            offset,
            size,
            step,
        }
    }

    pub fn u8_ptr(&self) -> *const u8 {
        self.u8_ptr
    }

    pub fn mask(&self) -> usize {
        self.mask
    }

    pub fn offset(&self) -> usize {
        self.offset
    }

    pub fn size(&self) -> usize {
        self.size
    }

    pub fn step(&self) -> usize {
        self.step
    }
}

pub(crate) fn callback(fn_holder: &FnHolder, param: CallbackParams) {
    match fn_holder {
        FnHolder::Func(f) => f(param),
        FnHolder::FfiFunc(ffi) => ffi.callback(param),
        FnHolder::Lambda(f) => f(param),
    }
}
