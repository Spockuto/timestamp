use candid::Principal;
use core::num::NonZeroU32;
use getrandom::{register_custom_getrandom, Error};
use ic_stable_structures::memory_manager::{MemoryId, MemoryManager, VirtualMemory};
use ic_stable_structures::{DefaultMemoryImpl, StableLog};
use rand_chacha::rand_core::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;
use std::borrow::BorrowMut;
use std::cell::RefCell;
use std::time::Duration;
use uhlc::{HLCBuilder, HLC, NTP64};

type Memory = VirtualMemory<DefaultMemoryImpl>;
const SEEDING_INTERVAL: Duration = Duration::from_secs(3600);
const MY_CUSTOM_ERROR_CODE: u32 = Error::CUSTOM_START + 31;

thread_local! {
    static GLOBAL_TIMESTAMP: HLC =  HLCBuilder::new()
    .with_clock(clock)
    .with_max_delta(Duration::from_secs(1))
    .build();

    static RNG: RefCell<Option<ChaCha20Rng>> = const { RefCell::new(None) };

    static COUNTER: RefCell<u64> = const { RefCell::new(0_u64) };

    static MEMORY_MANAGER: RefCell<MemoryManager<DefaultMemoryImpl>> =
        RefCell::new(MemoryManager::init(DefaultMemoryImpl::default()));

    static LOG: RefCell<StableLog<String, Memory, Memory>> = RefCell::new(
        StableLog::init(
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(0))),
            MEMORY_MANAGER.with(|m| m.borrow().get(MemoryId::new(1))),
        ).unwrap());

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

fn method_1() {
    GLOBAL_TIMESTAMP.with(|mut time| {
        COUNTER.with(|c| {
            *c.borrow_mut() += 1;
            LOG.with_borrow_mut(|log| {
                log.append(&format!(
                    "Method 1 Current time : {:?} and counter value {}",
                    time.borrow_mut().new_timestamp(),
                    c.borrow()
                ))
                .expect("Failed to insert log");
            });
        });
    });
}

fn method_2() {
    GLOBAL_TIMESTAMP.with(|mut time| {
        COUNTER.with(|c| {
            *c.borrow_mut() += 1;

            LOG.with_borrow_mut(|log| {
                log.append(&format!(
                    "Method 2 Current time : {:?} and counter value {}",
                    time.borrow_mut().new_timestamp(),
                    c.borrow()
                ))
                .expect("Failed to insert log");
            });
        });
    });
}

#[ic_cdk::update]
fn queue() {
    let mut buf = [0_u8; 10];
    RNG.with_borrow_mut(|rng| rng.as_mut().unwrap().fill_bytes(&mut buf));
    for i in buf {
        if i % 2 == 0 {
            method_1();
            method_2();
        } else {
            method_2();
            method_1();
        }
    }
}

#[ic_cdk::update]
fn get_log(size: u64) -> Vec<String> {
    LOG.with_borrow(|log| {
        if log.len() < size {
            return vec![];
        }

        let mut store = vec![];
        for i in ((log.len() - size)..log.len()).rev() {
            store.push(log.get(i).unwrap());
        }
        store
    })
}
