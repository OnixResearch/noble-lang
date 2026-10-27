pub trait Port {
    fn invoke(&self);
}

pub fn direct(value: u32) -> u32 {
    value + 2
}

pub const STORED_FUNCTION: fn(u32) -> u32 = direct;
pub static STORED_STATIC_FUNCTION: fn(u32) -> u32 = direct;

pub struct Wrapper(pub u32);

impl Wrapper {
    pub fn construct(value: u32) -> Self {
        Self(value)
    }
}

pub fn observe<F: Fn(u32) -> u32>(
    generic: F,
    dynamic: &dyn Fn(u32) -> u32,
    pointer: fn(u32) -> u32,
    port: &dyn Port,
) -> u32 {
    let pure = |value: u32| value + 1;
    let output = pure(7);
    let filesystem = || {
        let _ = std::fs::read("octet-fixture-never-executed");
    };
    filesystem();
    let direct_fn = direct;
    let _ = direct_fn(output);
    let _ = STORED_FUNCTION(output);
    let _ = STORED_STATIC_FUNCTION(output);
    let _ = generic(output);
    let _ = dynamic(output);
    let _ = pointer(output);
    port.invoke();
    let _ = Wrapper::construct(output);
    output
}

pub fn call_generic_port<T: Port>(generic_port: &T) {
    generic_port.invoke();
}
