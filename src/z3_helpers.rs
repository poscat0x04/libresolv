use z3::{Config, Context, set_global_param};
use z3::ast::Int;

pub fn set_params() {
    set_global_param("parallel.enable",  "true");
}

pub fn default_config() -> Config {
    let mut cfg = Config::new();
    cfg
}

pub fn zero(ctx: &Context) -> Int {
    Int::from_i64(ctx, 0)
}

#[cfg(test)]
mod test{
    use z3::Context;
    use crate::z3_helpers::{default_config, set_params};

    #[test]
    fn test_build_context() {
        set_params();
        let cfg = default_config();
        let context = Context::new(&cfg);
    }
}