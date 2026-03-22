use std::any::Any;

pub trait AsAny {
    fn as_any_mut(&mut self) -> &mut dyn Any;
    fn as_any(&self) -> &dyn Any;
}

#[macro_export]
macro_rules! map {
    ( $( $key:expr => $val:expr ),* $(,)? ) => {{
        let mut _map = std::collections::HashMap::new();
        $(
            _map.insert($key, $val);
        )*
        _map
    }};
    () => {{
        std::collections::HashMap::new()
    }};
}
