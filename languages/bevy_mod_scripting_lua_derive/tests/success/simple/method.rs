use bevy::prelude::*;
use bevy_mod_scripting::api::*;

#[derive(ScriptProxy, Reflect)]
#[proxy(languages("lua"), derive(Clone))]
#[functions[
    #[lua(Method)]
    fn fn_returning_some_string(&self) -> String;

    #[lua(Method,output(proxy))]
    fn fn_returning_proxy(&self) -> Self;
]]
#[derive(Clone)]
pub struct MyStruct {
    some_string: String,
    me_vec: Vec<usize>,
}

impl MyStruct {
    pub fn fn_returning_some_string(&self) -> String {
        self.some_string.clone()
    }

    pub fn fn_returning_proxy(&self) -> Self {
        self.clone()
    }
}

pub fn main() {}