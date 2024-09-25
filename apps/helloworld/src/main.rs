#![no_std]
#![no_main]
#![feature(type_alias_impl_trait)]

extern crate async_axstd;
use async_axstd::sync::Mutex;
static A: Mutex<i32> = Mutex::new(23);

use core::time::Duration;

#[async_axstd::async_main]
async fn main() -> i32 {
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
    for i in 0..400 {
        async_axstd::println!("for test preempt {}", i);
    }
    0
}