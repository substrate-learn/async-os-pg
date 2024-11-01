#![no_std]
#![no_main]

#[macro_use]
extern crate user_apps;

#[no_mangle]
pub fn main() -> i32 {
    println!("Hello world 2!");
    0
}
