#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

extern crate async_axstd;
extern crate alloc;

use core::{future::Future, pin::Pin};
use alloc::boxed::Box;

#[used]
#[no_mangle]
static async_main: fn() -> BoxFut = keep_name;

type BoxFut = Pin<Box<dyn Future<Output = i32> + Send + 'static>>;

#[no_mangle]
fn keep_name() -> BoxFut {
    Box::pin(async {
        for i in 0..5 {
            let b = i;
            let a = async_axstd::thread::spawn(async move {
                async_axstd::println!("Hello from a thread! {:?}", async_axstd::thread::current().id());
                b
            }).join().await;
            async_axstd::println!("Thread returned: {:?}", a);
        }
        async_axstd::println!("Hello, world!");
        0
    })
}