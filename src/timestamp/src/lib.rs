use candid::Principal;
use core::num::NonZeroU32;
use getrandom::register_custom_getrandom;
use getrandom::Error;
use rand_chacha::rand_core::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;
use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::time::Duration;
use uhlc::HLCBuilder;
use uhlc::HLC;
use uhlc::NTP64;

const SEEDING_INTERVAL: Duration = Duration::from_secs(3600);

thread_local! {
    static TIMESTAMP_1: HLC =  HLCBuilder::new()
    .with_clock(clock)
    .with_max_delta(Duration::from_secs(0))
    .build();

    static TIMESTAMP_2: HLC =  HLCBuilder::new()
    .with_clock(clock)
    .with_max_delta(Duration::from_secs(1))
    .build();

    static RNG: RefCell<Option<ChaCha20Rng>> = RefCell::new(None);
}

async fn seed_randomness() {
    let (seed,): ([u8; 32],) = ic_cdk::call(Principal::management_canister(), "raw_rand", ())
        .await
        .expect("Failed to call the management canister");
    RNG.with_borrow_mut(|rng| *rng = Some(ChaCha20Rng::from_seed(seed)));
}

fn schedule_seeding(duration: Duration) {
    ic_cdk_timers::set_timer(duration, || {
        ic_cdk::spawn(async {
            seed_randomness().await;
            // Schedule reseeding on a timer with duration SEEDING_INTERVAL
            schedule_seeding(SEEDING_INTERVAL);
        })
    });
}
// Some application-specific error code
const MY_CUSTOM_ERROR_CODE: u32 = Error::CUSTOM_START + 31;
pub fn custom_randomness(buf: &mut [u8]) -> Result<(), getrandom::Error> {
    RNG.with_borrow_mut(|rng| match rng.as_mut() {
        Some(rand) => {
            rand.fill_bytes(buf);
            Ok(())
        }
        None => {
            let code = NonZeroU32::new(MY_CUSTOM_ERROR_CODE).unwrap();
            Err(Error::from(code))
        }
    })
}

register_custom_getrandom!(custom_randomness);

fn clock() -> uhlc::NTP64 {
    let time = Duration::from_nanos(unsafe { ic0::time() as u64 });
    NTP64::from(time)
}

#[ic_cdk::init]
fn init() {
    // Initialize randomness during canister install or reinstall
    schedule_seeding(Duration::ZERO);
}

#[ic_cdk::post_upgrade]
fn post_upgrade() {
    // Initialize randomness after a canister upgrade
    schedule_seeding(Duration::ZERO);
}

#[ic_cdk::update]
fn method_1() {
    TIMESTAMP_1.with(|mut time| {
        ic_cdk::println!(
            "Method 1 Current time : {:#?} ",
            time.borrow_mut().new_timestamp()
        );
    })
}

#[ic_cdk::update]
fn method_2() {
    TIMESTAMP_1.with(|mut time| {
        let t1 = time.borrow_mut().new_timestamp();
        TIMESTAMP_2.with(|mut time| {
            time.borrow_mut().update_with_timestamp(&t1).unwrap();
            ic_cdk::println!("Method 2 Current time : {:#?} ", t1);
        })
    })
}

#[ic_cdk::update]
fn queue() {
    for _ in 0..100 {
        // method 1 will always follow method 2
        // second timestamp accepts a drift a 1 second from first timestamp
        method_1();
        method_2();
    }
}
