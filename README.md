Timestamp [WIP]
==============

A PoC canister implementation to demonstrate journaling with a hybrid logical clock for timestamping and a `StableLog` store for persistent logging.


## Service
```
service : {
    // Enqueue 10 calls to 2 methods in random order
    "queue": () -> ();

    // Get the last n log entries
    "get_log": (nat64) -> (vec text);
}


```