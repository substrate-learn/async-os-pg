#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

extern crate async_axstd;
extern crate alloc;

use core::{future::Future, pin::Pin, time::Duration};
use alloc::boxed::Box;

#[used]
#[no_mangle]
static ASYNC_MAIN: fn() -> BoxFut = keep_name;

type BoxFut = Pin<Box<dyn Future<Output = i32> + Send + 'static>>;

use async_axstd::sync::Mutex;
static A: Mutex<i32> = Mutex::new(23);

#[no_mangle]
fn keep_name() -> BoxFut {
    Box::pin(async {
        let mut b = A.lock().await;
        async_axstd::println!("Mutex locked: {:?}", *b);
        *b = 34;
        // drop(b);
        let j = async_axstd::thread::spawn(async {
            let a = A.lock().await;
            async_axstd::println!("spawn Mutex locked: {:?}", *a);
            32
        }).join();
        async_axstd::thread::sleep(Duration::from_secs(1)).await;
        drop(b);
        let res = j.await.unwrap();
        async_axstd::println!("res {}", res);
        async_axstd::thread::sleep(Duration::from_secs(1)).await;
        0
    })
}