use crate::{ffi::FfiFunc, param::CallbackParams};

pub(crate) type NormalFunc = Box<dyn Fn(CallbackParams) + Send + 'static>;
pub(crate) type LambdaFunc = Box<dyn Fn(CallbackParams) + 'static>;

pub(crate) enum FnHolder {
    Func(NormalFunc),
    FfiFunc(Box<dyn FfiFunc>),
    Lambda(LambdaFunc),
}

pub(crate) fn callback(fn_holder: &FnHolder, param: CallbackParams) {
    match fn_holder {
        FnHolder::Func(f) => f(param),
        FnHolder::FfiFunc(ffi) => ffi.callback(param),
        FnHolder::Lambda(f) => f(param),
    }
}
